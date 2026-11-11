use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use cu::pre::*;
use pyo3::prelude::*;

use crate::interop;

/// Wrap an execution with a handle to send Python commands
pub fn execute_with_cmd_host<F: FnOnce(PyCmdSender) -> cu::Result<u32> + Send + 'static>(
    f: F,
) -> PyCmdHost {
    let (cmd_send, cmd_recv) = mpsc::channel();
    let (res_send, res_recv) = mpsc::channel();
    let task_thread = Some(thread::spawn(move || f(PyCmdSender { cmd_send, res_recv })));
    let cmd_host = RustPyCmdHost {
        cmd_recv,
        res_send,
        task_thread,
    };
    PyCmdHost::new(cmd_host)
}

/// State for sending commands to Python to run.
///
/// This is easier than implementing callback from Rust to python,
/// because Python objects are tied to the interpreter, and error
/// handling gets trickier too. Ideally we should never need to call
/// Python from Rust other than interop with external python libraries
/// like asm-differ
#[derive(Debug)]
pub struct RustPyCmdHost {
    /// Channel for python thread to receive commands
    cmd_recv: mpsc::Receiver<PyCmd>,
    /// Channel for python to send result back
    res_send: mpsc::Sender<(bool, String)>,
    /// Thread running Rust that may send commands to python
    task_thread: Option<JoinHandle<cu::Result<u32>>>,
}
interop::make_handle_class! {
    /// Host for receiving commands from Rust
    #[handle(RustPyCmdHost, module = "viking._core")]
    pub mut struct PyCmdHost;
}

#[pymethods]
impl PyCmdHost {
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
    pub fn send_result(&self, success: bool, result: String) {
        let _ = self.res_send.send((success, result));
    }

    /// Called after `recv()` returns `None` to wait for the Rust side
    /// to finish running
    pub fn join(&mut self) -> PyResult<u32> {
        if self.0 == 0 {
            return Ok(0);
        }
        let Some(thread) = self.task_thread.take() else {
            return Ok(0);
        };
        fn join_imp(thread: JoinHandle<cu::Result<u32>>) -> cu::Result<u32> {
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

/// Command send from Rust to Python to execute some Python in the middle of Rust
#[derive(Debug, Clone, Display)]
#[display("{self:?}")]
#[pyclass(str, from_py_object)]
pub enum PyCmd {
    /// Run asm-differ with the given arguments
    AsmDiffer(Vec<String>),
    /// Print optional arguments of asm-differ
    AsmDifferOptionalArgsHelp(),
}

/// Sender facade for Rust to dispatch PyCmd. Dropping
/// the sender marks the end of the API lifetime
pub struct PyCmdSender {
    cmd_send: mpsc::Sender<PyCmd>,
    res_recv: mpsc::Receiver<(bool, String)>,
}
impl PyCmdSender {
    /// Format help for optional arguments for asm-differ
    pub fn asm_differ_optional_arguments_help(&self) -> cu::Result<String> {
        cu::check!(
            self.do_send(PyCmd::AsmDifferOptionalArgsHelp()),
            "failed to format asm-differ optional arguments help"
        )
    }
    /// Call asm-differ with the given args
    pub fn asm_differ(&self, args: impl IntoIterator<Item = impl Into<String>>) -> cu::Result<()> {
        let args = args.into_iter().map(Into::into).collect();
        let args_debug = format!("{args:?}");
        let cmd = PyCmd::AsmDiffer(args);
        cu::check!(
            self.do_send(cmd),
            "asm-differ command failed, args: {args_debug}"
        )?;
        Ok(())
    }

    fn do_send(&self, cmd: PyCmd) -> cu::Result<String> {
        if self.cmd_send.send(cmd).is_err() {
            cu::bail!("unexpected: python cmd host's command channel is disconnected");
        }
        match self.res_recv.recv() {
            Err(_) => {
                cu::bail!("unexpected: python cmd host's result channel is disconnected");
            }
            Ok((ok, response)) => {
                if !ok {
                    cu::bail!("python cmd returned error: {response}")
                } else {
                    Ok(response)
                }
            }
        }
    }
}
