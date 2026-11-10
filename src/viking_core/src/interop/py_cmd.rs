use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use cu::pre::*;
use pyo3::prelude::*;

use crate::interop;

/// Wrap an execution with a handle to send Python commands
pub fn execute_with_cmd_host<F: FnOnce(PyCmdSender) -> cu::Result<()> + Send + 'static>(
    f: F,
) -> PyCmdHostHandle {
    let (cmd_send, cmd_recv) = mpsc::channel();
    let (err_send, err_recv) = mpsc::channel();
    let task_thread = Some(thread::spawn(move || f(PyCmdSender { cmd_send, err_recv })));
    let cmd_host = RustPyCmdHost {
        cmd_recv,
        err_send,
        task_thread,
    };
    PyCmdHostHandle(Box::into_raw(Box::new(cmd_host)) as usize)
}


/// State for sending commands to Python to run.
///
/// This is easier than implementing callback from Rust to python,
/// because Python objects are tied to the interpreter, and error
/// handling gets trickier too. Ideally we should never need to call
/// Python from Rust other than interop with external python libraries
/// like asm-differ
pub struct RustPyCmdHost {
    /// Channel for python thread to receive commands
    cmd_recv: mpsc::Receiver<PyCmd>,
    /// Channel for python to send result back
    err_send: mpsc::Sender<Option<String>>,
    /// Thread running Rust that may send commands to python
    task_thread: Option<JoinHandle<cu::Result<()>>>,
}
interop::make_handle_class!(RustPyCmdHost, PyCmdHostHandle mut);

#[pymethods]
impl PyCmdHostHandle {
    /// Blocks Python code until a command is received, or None
    /// if sender is dropped
    pub fn recv(&self) -> Option<PyCmd> {
        if self.0 == 0 {
            return None;
        }
        self.cmd_recv.recv().ok()
    }

    /// Send a result back to Rust. None means success, otherwise
    /// sends an error message string
    pub fn send_result(&self, result: Option<String>) {
        let _ = self.err_send.send(result);
    }

    pub fn join(&mut self) -> PyResult<()> {
        if self.0 == 0 {
            return Ok(());
        }
        let Some(thread) = self.task_thread.take() else {
            return Ok(());
        };
        fn join_imp(thread: JoinHandle<cu::Result<()>>) -> cu::Result<()> {
            match thread.join() {
                Ok(x) => x,
                Err(e) => {
                    let info = cu::best_effort_panic_info(&e);
                    cu::bail!("rust panicked: {info}");
                }
            }
        }
        let result = join_imp(thread);
        // drop the host
        let _ = unsafe { self.claim_by_ref() };
        self.0 = 0;
        interop::into_pyresult(result)
    }
}

#[derive(Debug, Clone, Display)]
#[display("{self:?}")]
#[pyclass(str, from_py_object)]
pub enum PyCmd {
    /// Run asm-differ with the given arguments
    AsmDiffer(Vec<String>),
}

/// Sender facade for Rust to dispatch PyCmd. Dropping
/// the sender marks the end of the API lifetime
pub struct PyCmdSender {
    cmd_send: mpsc::Sender<PyCmd>,
    err_recv: mpsc::Receiver<Option<String>>,
}
impl PyCmdSender {
    /// Call asm-differ with the given args
    pub fn asm_differ(&self, args: impl IntoIterator<Item = impl Into<String>>) -> cu::Result<()> {
        let args = args.into_iter().map(Into::into).collect();
        let args_debug = format!("{args:?}");
        let cmd = PyCmd::AsmDiffer(args);
        cu::check!(
            self.do_send(cmd),
            "ams-differ command failed, args: {args_debug}"
        )
    }

    fn do_send(&self, cmd: PyCmd) -> cu::Result<()> {
        if self.cmd_send.send(cmd).is_err() {
            cu::bail!("unexpected: python cmd host's command channel is disconnected");
        }
        match self.err_recv.recv() {
            Err(_) => {
                cu::bail!("unexpected: python cmd host's result channel is disconnected");
            }
            Ok(Some(t)) => {
                cu::bail!("python cmd returned error: {t}")
            }
            Ok(None) => Ok(()),
        }
    }
}
