# Contributing Guidelines

## Setup

Please clone the repo and install the tools:

- [UV](https://docs.astral.sh/uv/getting-started/installation/)
- [Rust](https://rustup.rs)
- [Task](https://taskfile.dev) the task runner

## Development workflow

Please always use the `task` commands instead of running
`uv` or `cargo` directly for the optimal performance. You can
read `Taskfile.yml` for more information on how these commands work.

Run `task --list` to list all available commands.

Common workflows:
- `task install`: Syncs dependencies and re-generate the Python bindings from Rust.
  Rerun whenever the repo is updated or need to re-generate the Python bindings from Rust.
- `task run -- [ARGS]..`: Run the viking CLI. The Rust code is automatically rebuilt.
  You may need to specify the `VIKING_CONFIG` environment variable to point to the
  project you are testing with. (e.g. `VIKING_CONFIG=path/to/botw/tools/config.toml task run -- check`)
- `task check`: Checks for formatting and linting issues
- `task fix`: Fix formatting issues

Please run `task check` and `task fix` to fix trivial issues before publishing/updating
your PR, so the review process can be more streamlined.

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

Only Python code can call Rust. Rust cannot call Python directly. This design
decision is made so that Python doesn't bring overhead to paths that don't
need Python (even if other paths in the same entry point needs Python, like `check`).

When Rust needs to call Python, it must do so through `PyCmd`, which serializes
a JSON Payload and invokes the `py_json_cmd` command in the Python CLI.

To add a new Python call:
- Add the command to [`PyCmd`](../src/viking_core/src/python/py_cmd.rs).
- Add the Rust API to call the command in the [`python`](../src/viking_core/src/python/driver.rs) in Rust.
  This is what the Rust code calls.
- Run `task install` to regenerate the Python bindings (this is only needed
  so your editor picks up the new command you added).
- Add the Python code that will handle the command to [`run_json_command`](../src/viking/core.py).

### "Global" state in Rust

The Rust side should be free of process-wide singleton to support the upcoming Python multi-interpreter feature,
where a single process may host multiple Python interpreters. This means you need to avoid something
like `static MY_STATE: LazyLock<...> = ...` in Rust.

The bad news is that `pyo3` does not current support declaring per-module state. We work around this
by allocating the global state in Rust once during module init, and pass (leak) the handle to Python
and store it as a module scope variable. The Rust functions that depend on this global state must
take it as an argument from the Python side. See [`core.py`](../src/viking/core.py).

This global state should not be exposed in the Python public API. There should be a wrapper
that calls the `_core` API internally and passing in `core_state`.
