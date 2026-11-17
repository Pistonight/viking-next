use std::ffi::{OsStr, OsString};
use std::process::{Command, ExitStatus, Output};
use std::sync::Mutex;

use cu::pre::*;
use pyo3::prelude::*;

/// The command to execute python intrepreter that has viking installed
static VIKING_PYTHON_COMMAND: Mutex<Option<Vec<OsString>>> = Mutex::new(None);

/// Set the Python command to execute viking.
///
/// Usually if viking is invoked from it's Python CLI, this is automatically
/// called as part of the initialization. However, for performance reason,
/// viking also ships its CLI as native binaries, which is faster on paths
/// when no Python code needs to be executed.
///
/// However, in case Python code does need to run, for example opening ASM
/// diffing as part of the `check` tool, the native binary needs to know
/// how to invoke the viking Python CLI.
///
/// For example, this should be something like `["uv", "run", "viking"]` for UV setup,
/// or `["python3", "-m", "viking"]` for raw Python setup.
pub fn set_python_command<I: IntoIterator<Item = impl AsRef<OsStr>>>(command: I) {
    let mut s_command = VIKING_PYTHON_COMMAND
        .lock()
        .expect("failed to lock VIKING_PYTHON_COMMAND");
    *s_command = Some(
        command
            .into_iter()
            .map(|x| x.as_ref().to_os_string())
            .collect(),
    )
}

/// Set VIKING_PYTHON_COMMAND from VIKING_PYTHON_COMMAND environment variable
#[cu::context(
    "failed to set python command from VIKING_PYTHON_COMMAND environment variable; this environment variable is required when running viking native binaries"
)]
fn set_python_command_from_env() -> cu::Result<()> {
    // for now we don't support quoting... for simplicity
    let command = cu::env_var("VIKING_PYTHON_COMMAND")?;
    if command.is_empty() {
        cu::bail!("VIKING_PYTHON_COMMAND is empty");
    }
    set_python_command(command.split_whitespace());
    Ok(())
}

fn get_python_command(args: Vec<OsString>) -> cu::Result<(OsString, Vec<OsString>)> {
    let (arg0, command) = loop {
        let command = VIKING_PYTHON_COMMAND
            .lock()
            .expect("failed to lock VIKING_PYTHON_COMMAND");
        match &*command {
            None => {
                drop(command);
                set_python_command_from_env()?;
                continue;
            }
            Some(x) => {
                let mut command = Vec::with_capacity(x.len() + args.len());
                let mut iter = x.iter();
                let arg0 = cu::check!(
                    iter.next(),
                    "viking python command is missing executable (arg0)"
                )?;
                command.extend(iter.cloned());
                command.extend(args);
                break (arg0.clone(), command);
            }
        }
    };
    Ok((arg0, command))
}

/// Command send from Rust to Python to execute some Python in the middle of Rust
#[derive(Debug, Clone, Display, Serialize, Deserialize)]
#[display("{self:?}")]
#[pyclass(str, from_py_object)]
#[doc(hidden)]
pub enum PyCmd {
    /// Run asm-differ with the given arguments
    AsmDiffer(Vec<String>),
    /// Print optional arguments of asm-differ
    AsmDifferOptionalArgsHelp(),
}
impl PyCmd {
    #[cu::context("failed to build PyCmd")]
    pub fn build(&self) -> cu::Result<PyCmdExecutor> {
        let json = json::stringify(self)?;
        let args = vec!["py-json-call".into(), json.into()];
        Ok(PyCmdExecutor { args })
    }
}

#[doc(hidden)]
pub struct PyCmdExecutor {
    args: Vec<OsString>,
}
impl PyCmdExecutor {
    pub fn exec_captured(self, check: bool) -> cu::Result<Output> {
        let (arg0, command) = get_python_command(self.args)?;
        let child = cu::check!(
            Command::new(&arg0).args(&command).output(),
            "failed to spawn '{}' with args '{:?}'",
            arg0.display(),
            command
        )?;
        if check && !child.status.success() {
            cu::bail!("viking python process exited with {}", child.status);
        }
        Ok(child)
    }
    pub fn exec_inherit(self, check: bool) -> cu::Result<ExitStatus> {
        let (arg0, command) = get_python_command(self.args)?;
        let status = cu::check!(
            Command::new(&arg0).args(&command).status(),
            "failed to spawn '{}' with args '{:?}'",
            arg0.display(),
            command
        )?;
        if check && !status.success() {
            cu::bail!("viking python process exited with {status}");
        }
        Ok(status)
    }
}
