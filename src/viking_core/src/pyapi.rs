use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::sync::mpsc;

use pyo3::prelude::*;
use cu::pre::*;

use crate::config::{self, Config};

/// Global (per-module) state of the module
///
/// Pyo3 does not yet support per-module state (for multi-interpreter),
/// so we leak a pointer to let python hold the
pub struct State {
    /// Configuration file
    pub config: Config,
}
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct StateHandle(usize);
impl std::ops::Deref for StateHandle {
    type Target = State;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.0 as *const State) }
    }
}
#[pymethods]
impl StateHandle {
    pub fn config(&self) -> Config {
        self.config.clone()
    }
}

/// Initialize the rust extension module
#[pyfunction]
pub fn init_core_state() -> PyResult<StateHandle> {
    // this isn't per-module state but not really anything we can do
    // since it's in 3rd party state, but shouldn't really matter
    crate::ui::init_prompt_settings();

    let config = into_pyresult(config::load_config())?;
    let state = Box::new(State { config });
    Ok(StateHandle(Box::into_raw(state) as usize))
}

/// Finalize the rust extension module
#[pyfunction]
pub fn fini_core_state(state: StateHandle) {
    let _ = unsafe { Box::from_raw(state.0 as *mut State) };
}

/// Find a tool based on repo config
#[pyfunction]
pub fn find_tool(state: StateHandle, tool: &str) -> PyResult<PathBuf> {
    into_pyresult(state.config.repo.find_tool(tool))
}

pyo3::create_exception!(viking, CoreError, pyo3::exceptions::PyRuntimeError);
pub fn into_pyresult<T>(result: cu::Result<T>) -> PyResult<T> {
    result.map_err(|e| {
        use std::backtrace::BacktraceStatus;
        let error_string = match e.backtrace().status() {
            BacktraceStatus::Disabled => {
                format!("[Set RUST_LIB_BACKTRACE=1 to view backtrace in Rust]\n{e:?}")
            }
            BacktraceStatus::Captured => {
                // clean up the backtrace
                let error_raw_string = format!("{e:?}");
                let mut lines = vec![];
                lines.push("");
                let mut found_stack_trace = false;
                for line in error_raw_string.lines() {
                    lines.push(line);
                    if line.starts_with("Stack backtrace:") {
                        found_stack_trace = true;
                        continue;
                    }
                    // the entry point from python must be one of the __pyfunction bindings
                    if found_stack_trace
                        && line.contains(": viking_core::")
                        && line.contains("__pyfunction")
                    {
                        lines.push("      <python interop frames are hidden>");
                        break;
                    }
                }
                lines.join("\n")
            }
            _ => {
                format!("\n{e:?}")
            }
        };
        CoreError::new_err(error_string)
    })
}

/// State for sending commands to Python to run.
///
/// This is easier than implementing callback from Rust to python,
/// because Python objects are tied to the interpreter, and error
/// handling gets trickier too. Ideally we should never need to call
/// Python from Rust other than interop with external python libraries
/// like asm-differ
pub struct PyCmdHost {
    /// Channel for python thread to receive commands
    cmd_recv: mpsc::Receiver<PyCmd>,
    /// Channel for python to send result back
    err_send: mpsc::Sender<Option<String>>,
    /// Thread running Rust that may send commands to python
    task_thread: Option<JoinHandle<cu::Result<()>>>,
}
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct PyCmdHostHandle(usize);
impl std::ops::DerefMut for PyCmdHostHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.0 as *mut PyCmdHost) }
    }
}

impl std::ops::Deref for PyCmdHostHandle {
    type Target = PyCmdHost;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.0 as *const PyCmdHost) }
    }
}
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
        let _ = unsafe { Box::from_raw(self.0 as *mut PyCmdHost) };
        self.0 = 0;
        into_pyresult(result)
    }
}

#[derive(Debug, Clone, Display)]
#[display("{self:?}")]
#[pyclass(str, from_py_object)]
pub enum PyCmd {
    /// Run asm-differ with the given arguments
    AsmDiffer(Vec<String>),
}

/// Wrap an execution with a handle to send Python commands
pub fn execute_with_cmd_host<F: FnOnce(PyCmdApi) -> cu::Result<()> + Send + 'static>(f: F) -> PyCmdHostHandle {
    let (cmd_send, cmd_recv) = mpsc::channel();
    let (err_send, err_recv) = mpsc::channel();
    let task_thread = Some(thread::spawn(move || {
        f(PyCmdApi{cmd_send, err_recv})
    }));
    let cmd_host = PyCmdHost { cmd_recv, err_send, task_thread  };
    PyCmdHostHandle(Box::into_raw(Box::new(cmd_host)) as usize)
}

/// Sender facade for Rust to dispatch PyCmd. Dropping
/// the sender marks the end of the API lifetime
pub struct PyCmdApi{
    cmd_send: mpsc::Sender<PyCmd>,
    err_recv: mpsc::Receiver<Option<String>>,
}
impl PyCmdApi {
    pub fn asm_differ(&self, args: impl IntoIterator<Item=impl Into<String>>) -> cu::Result<()> {
        let args = args.into_iter().map(Into::into).collect();
        let args_debug = format!("{args:?}");
        let cmd = PyCmd::AsmDiffer(args);
        cu::check!(self.do_send(cmd), "ams-differ command failed, args: {args_debug}")
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
            Ok(None) => Ok(())
        }
    }
}

