use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use std::sync::Mutex;

use cu::pre::*;
use pyo3::prelude::*;

use crate::config;

/// The command to execute the Python interpreter that has viking installed
///
/// This is process-wide state. It does not need to be per-interpreter,
/// since a new Python process is spawned every time Rust calls Python,
/// so it doesn't matter if multiple interpreters set it differently
static VIKING_PYTHON_COMMAND_PARAMS: Mutex<Option<PythonCommandParams>> = Mutex::new(None);

/// Convenience wrapper to call [`set_python_command`]
/// without `cwd` or `envs`.
///
/// See [`set_python_command`]
#[inline(always)]
pub fn set_python_program_args(
    program: impl Into<String>,
    args: impl IntoIterator<Item = impl Into<String>>,
) -> cu::Result<()> {
    let program = program.into();
    let args = args.into_iter().map(Into::into).collect();
    set_python_command(PythonCommandParams {
        program,
        args,
        cwd: None,
        envs: vec![],
    })
}

/// Set the Python command to execute viking Python CLI.
///
/// If viking is invoked from its Python CLI, this is called automatically
/// as part of the initialization. However, for performance reasons,
/// viking also ships a Rust CLI, which is faster on paths where no Python
/// code needs to be executed.
///
/// When Python code does need to run, for example opening ASM
/// diffing as part of the `check` tool, the native binary needs to know
/// how to invoke the viking Python CLI.
///
/// ## Example
/// ```rust,no_run
/// use viking::python;
///
/// # fn main() -> cu::Result<()> {
/// python::set_python_command(python::PythonCommandParams {
///     program: "uv".to_string(),
///     args: vec!["run".to_string(), "viking".to_string()],
///     cwd: None,
///     envs: vec![
///         ("UV_PROJECT".to_string(), "path/to/tools".to_string())
///     ]
/// })?;
/// # Ok(()) }
/// ```
///
/// ## Environment override
/// The spawned process will have these environment variables set regardless
/// of the command params:
/// - `VIKING_CONFIG`: Set to the config path discovered.
/// - `VIKING_PYTHON_COMMAND_PROGRAM`, `VIKING_PYTHON_COMMAND_ARGS` and `VIKING_PYTHON_COMMAND_CWD`:
///   set to the ones being used to invoke the Python CLI.
///
/// ## Detection from environment variables
///
/// The Python command params can also be set from environment variables, then
/// no manual `set_python_command` is needed:
///
/// - `VIKING_PYTHON_COMMAND_PROGRAM`: The `program` field
/// - `VIKING_PYTHON_COMMAND_ARGS`: The `args` field (split using shell quoting)
/// - `VIKING_PYTHON_COMMAND_CWD`: The `cwd` field
/// - Other environment variables: set them for the viking Rust process,
///   and they will be inherited by the child process.
///
/// ## Automatic configuration
///
/// For projects using UV, the params are set automatically if:
/// - `uv` is found in `PATH`
/// - There is no `VIKING_PYTHON_COMMAND_PROGRAM` environment variable set.
/// - There is a `uv.lock` file next to either the config file, or the
///   current executable (discovered in this order).
///
/// In this case, the params will be automatically configured as:
/// - `program`: `uv` found in `PATH`
/// - `args`: `["run", "viking"]`
/// - `cwd`: `None`
/// - `envs`:
///   - `UV_PROJECT`: Directory containing `uv.lock`
///
#[cu::context("failed to set viking python command")]
pub fn set_python_command(mut params: PythonCommandParams) -> cu::Result<()> {
    params.envs.retain(|(k, _)| {
        !matches!(
            k.as_str(),
            "VIKING_CONFIG"
                | "VIKING_PYTHON_COMMAND_PROGRAM"
                | "VIKING_PYTHON_COMMAND_ARGS"
                | "VIKING_PYTHON_COMMAND_CWD"
        )
    });
    let config = match config::discover_config_optional() {
        None => Some("0".to_string()),
        Some(Err(_)) => None,
        Some(Ok(x)) => Some(x.into_utf8()?),
    };
    if let Some(config) = config {
        params.envs.push(("VIKING_CONFIG".to_string(), config));
    }
    params.envs.push((
        "VIKING_PYTHON_COMMAND_PROGRAM".to_string(),
        params.program.to_string(),
    ));
    if !params.args.is_empty() {
        params.envs.push((
            "VIKING_PYTHON_COMMAND_ARGS".to_string(),
            shell_words::join(&params.args),
        ));
    }
    if let Some(cwd) = &params.cwd {
        params
            .envs
            .push(("VIKING_PYTHON_COMMAND_CWD".to_string(), cwd.to_string()));
    }
    let mut s_params = VIKING_PYTHON_COMMAND_PARAMS
        .lock()
        .expect("failed to lock VIKING_PYTHON_COMMAND_PARAMS");
    *s_params = Some(params);
    Ok(())
}

#[cu::context("failed to set viking python command automatically from environment")]
fn set_python_command_automatic() -> cu::Result<()> {
    let program = cu::env_var("VIKING_PYTHON_COMMAND_PROGRAM").unwrap_or_default();
    let args = cu::env_var("VIKING_PYTHON_COMMAND_ARGS").unwrap_or_default();
    let cwd = cu::env_var("VIKING_PYTHON_COMMAND_CWD").unwrap_or_default();
    if !program.is_empty() {
        let args = cu::check!(
            shell_words::split(&args),
            "failed to parse VIKING_PYTHON_COMMAND_ARGS"
        )?;
        let cwd = (!cwd.is_empty()).then_some(cwd);
        return set_python_command(PythonCommandParams {
            program,
            args,
            cwd,
            envs: vec![],
        });
    }
    if let Ok(uv) = cu::which("uv") {
        cu::debug!("found uv, trying to set python command for uv");
        if let Ok(uv) = uv.into_utf8() {
            if let Some(Ok(config)) = config::discover_config_optional() {
                if let Some(dir) = config.parent() {
                    let uv_lock = dir.join("uv.lock");
                    if uv_lock.is_file() {
                        cu::debug!("found uv.lock next to config: {}", uv_lock.display());
                        return set_python_command_automatic_for_uv(uv, dir);
                    }
                }
            }
            if let Ok(executable) = std::env::current_exe() {
                if let Some(dir) = executable.parent() {
                    let uv_lock = dir.join("uv.lock");
                    if uv_lock.is_file() {
                        cu::debug!("found uv.lock next to executable: {}", uv_lock.display());
                        return set_python_command_automatic_for_uv(uv, dir);
                    }
                }
            }
            cu::debug!("did not find uv.lock");
        } else {
            cu::debug!("uv executable is not utf8");
        }
    }

    cu::bail!("please refer to the setup documentation: https://pistonight.github.io/viking-next/setup.html");
}

fn set_python_command_automatic_for_uv(uv: String, project_dir: &Path) -> cu::Result<()> {
    let project = project_dir.as_utf8()?.to_string();
    set_python_command(PythonCommandParams {
        program: uv,
        args: vec!["run".to_string(), "viking".to_string()],
        cwd: None,
        envs: vec![("UV_PROJECT".to_string(), project)],
    })
}

/// Parameters for Rust to call Python
///
/// See [`set_python_command`]
#[derive(Debug, Clone)]
pub struct PythonCommandParams {
    /// The path of the program
    pub program: String,
    /// The args to invoke the program with to run viking
    pub args: Vec<String>,
    /// The current directory to invoke the program
    pub cwd: Option<String>,
    /// The environment to set
    pub envs: Vec<(String, String)>,
}

fn make_python_command(args: &[String]) -> cu::Result<Command> {
    let mut is_set = false;
    loop {
        let params = VIKING_PYTHON_COMMAND_PARAMS
            .lock()
            .expect("failed to lock VIKING_PYTHON_COMMAND_PARAMS");
        let Some(params) = &*params else {
            drop(params);
            if is_set {
                cu::bail!("unexpected: python command should already be set, this is a bug");
            }
            set_python_command_automatic()?;
            is_set = true;
            continue;
        };
        let mut builder = Command::new(&params.program);
        builder
            .args(params.args.iter().chain(args.iter()))
            .envs(params.envs.iter().map(|(k, v)| (k, v)));
        if let Some(cwd) = &params.cwd {
            builder.current_dir(cwd);
        }
        return Ok(builder);
    }
}

/// Command sent from Rust to Python to execute some Python in the middle of Rust
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
        let args = vec!["py-json-call".into(), json];
        Ok(PyCmdExecutor { args })
    }
}

#[doc(hidden)]
pub struct PyCmdExecutor {
    args: Vec<String>,
}
impl PyCmdExecutor {
    pub fn exec_captured(self, check: bool) -> cu::Result<Output> {
        let mut command = make_python_command(&self.args)?;
        let child = cu::check!(
            command.output(),
            "failed to spawn '{}' with args '{:?}'",
            command.get_program().display(),
            command.get_args().collect::<Vec<_>>()
        )?;
        if check && !child.status.success() {
            cu::bail!("viking python process exited with {}", child.status);
        }
        Ok(child)
    }
    pub fn exec_inherit(self, check: bool) -> cu::Result<ExitStatus> {
        let mut command = make_python_command(&self.args)?;
        let status = cu::check!(
            command.status(),
            "failed to spawn '{}' with args '{:?}'",
            command.get_program().display(),
            command.get_args().collect::<Vec<_>>()
        )?;
        if check && !status.success() {
            cu::bail!("viking python process exited with {status}");
        }
        Ok(status)
    }
}
