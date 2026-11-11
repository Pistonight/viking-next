
## Setup

Please clone the repo and install the tools:

- [UV](https://docs.astral.sh/uv/getting-started/installation/)
- [Rust](https://rustup.rs)
- [Task](https://taskfile.dev) the task runner

## Development workflow

Please always use the `task` commands instead of running
`uv` or `cargo` directly for the optimal performance. You can
read `Taskfile.yml` for more information on how these commands work.

- `task install`: Syncs dependencies and re-generate the Python bindings from Rust.
  Rerun whenever the repo is updated or need to re-generate the Python bindings from Rust.
- `task run [ARGS]..`: Run the viking CLI. The Rust code is automatically rebuilt.
  You may need to specify the `VIKING_CONFIG` environment variable to point to the
  project you are testing with. (e.g. `VIKING_CONFIG=path/to/botw/tools/config.toml task run -- check`)
- `task check`: Checks for formatting and linting issues
- `task fix`: Fix formatting issues

## Architecture

### Python and Rust split

The project codebase is split into the Python API (`src/viking`) and the Rust implementation
(`src/viking_core`). In most cases, the Rust side does the heavy-lifting, and the Python side
exposes a nice API for downstream projects to integrate into their tooling scripts.

The rule of thumb when choosing if something should be in Rust or Python is:

- **Implement in Rust and expose a Python API** when:
  - Both Rust and Python code depends on it.
  - Functionality is complex, such as parsing/validation.
  - Functionality can benefit from the performance of native code.
- **Implement in Python directly** when:
  - Functionality is simple and easier to implement in Python (such as spawning some tools)
  - Rust side does NOT depend on it.

### Python-Rust call direction

The last point is important: Currently, the Rust side does not callback to the Python side.
Once Python calls a Rust function, the control should stay inside Rust. This is a design decision
to simplify error handling, traceback reporting, and avoid the complexity with handing the Python
interpreter and references in Rust.

However, in some scenarios, it is not avoidable to require Rust code to call Python code, for example
to invoke some library that's only in Python. We solve this with [the command pattern](src/viking_core/src/interop/py_cmd.rs).
When Python calls a Rust function that requires calling back to Python at some point, the Rust function
runs on a separate thread, and the Python calling thread blocks the interpreter to wait for commands from
the Rust side. When the Rust side issues a command, the control switches to the thread running the Python
interpreter and the Rust thread blocks until the Python side finishes and returns a result or an exception.

### "Global" state

The Rust side should be free of process-wide singleton to support the upcoming Python multi-interpreter feature,
where a single process may host multiple Python interpreters. This means you need to avoid something
like `static MY_STATE: LazyLock<...> = ...` in Rust.

The bad news is that `pyo3` does not current support declaring per-module state. We work around this
by allocating the global state in Rust once during module init, and pass (leak) the handle to Python
and store it as a module scope variable. The Rust functions that depend on this global state must
take it as an argument from the Python side. See [`core.py`](src/viking/src/core.py).

### Python API structure

The Rust bindings are generated at the `viking._core` module. This module is meant to be low-level
FFI bindings and is not meant to be called directly by downstream projects. Instead, `viking.core`,
`viking.ui` and `viking.fl` expose higher-level, Python-friendly facades that calls into `_core`
under the hood.
