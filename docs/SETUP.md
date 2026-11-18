# Setup nx-decomp-tools-viking

> Note: the tools support Linux x64, with partial support for Linux arm64 and macOS arm64.
> The ARM platforms should work, but they aren't tested as much.

Before getting started, you need to decide on 2 things about how your project
will depend on this tool:

1. **Source vs. binary release**: Since this tool is very niche, it's not
   published to well-known package registries like PyPI or crates.io.
   Everything is hosted on GitHub instead, so you will need to point your tooling
   to GitHub (e.g. in `pyproject.toml`, `Cargo.toml`, etc). This repo provides
   both sources and pre-built binaries.
   - Install from *sources*:
     - Pros: Works nicely with most Python tools, like `uv` and the OG `pip`.
     - Cons: Requires a Rust toolchain to install, and contributors to your project
       need to compile the tools when installing, which is a bit slow.
   - Install from *pre-built binaries*:
     - Pros: Faster setup for your contributors. Does not require Rust.
     - Cons:
         - `pip` can install pre-built wheels from a GitHub-hosted link,
           but it will always prioritize PyPI, which is a potential supply-chain attack
           vector (i.e. if someone name-squats `nx-decomp-tools-viking`). `uv` does
           not have this issue.
         - The pre-built binaries target recent operating systems and might not work
           on old computers.

2. **Python vs. Rust CLI**: Most tools are written in Rust, wrapped with a Python
   API to make it easier to write scripts. For the CLI tools, you can either call
   them from the Python package (e.g. `uv run viking` or `python3 -m viking`),
   or call a native Rust binary directly.
     - Calling the *Python CLI*:
       - Pros: No additional setup. Your existing workflows can call
         `uv run viking ...` or `python3 -m viking ...` directly, or
         call the Python CLI API programmatically.
       - Cons: There is the overhead of starting the Python interpreter,
         even for pure Rust tools. For fast inner-loop workflows
         like checking whether a single function matches, the difference is
         noticeable if you are sensitive to performance.
     - Calling the *Rust CLI*:
       - Pros: The native CLI only spawns a Python process if and when needed,
         which makes it a bit faster.
       - Cons: It needs a lot of additional setup, including
         installing the native binary in addition to the Python package.
         The Python package is still required for tools that are implemented
         in Python (such as `asm-differ`), and additional setup is needed
         to allow the Rust CLI to call the Python CLI.

## Dependencies

- Git: A few dependencies are resolved from GitHub, so you
  need Git even if you are installing pre-built binaries.
- Python: [`uv`](https://docs.astral.sh/uv/getting-started/installation/) is the recommended project manager for Python,
  but any tool that works with `pyproject.toml` should work. Python 3.14+ is required
  (the version requirement is also specified in `pyproject.toml`).
- Rust: *optional - only needed if you are installing from source*.
  The recommended way to set up Rust is through the official toolchain manager, [Rustup](https://rustup.rs).
- `nx2elf`: Required for converting NSO to ELF.
- `llvm-objdump`: Required for disassembly diffing.

(`nx2elf` and `llvm-objdump` may be included with the installation in the future.)

## Configuration

The tool reads a single config file, which is either:
- `tools/config.toml` in your repo (searched for in the current directory and its parents), or
- the file that the `VIKING_CONFIG` environment variable points to,
  either as an absolute path or relative to the current working directory.

A minimal config file looks like this:
```toml
[repo]
# root_path: relative to the directory containing the config file.
# All other paths in the config file are relative to the root path
root_path = ".."
# data_path: directory that stores the original ELF, the file list,
# and other data files that might be added in the future
data_path = "data"
# build_path: ideally a CMake build directory that contains the output
# of your project
build_path = "build"

# file_list: name of the YAML file list, in the data path
file_list = "file_list.yml"
# data_symbols: names of the data symbol CSVs, in the data path
data_symbols = [ "data_symbols.csv" ]
# build_target: name of the executable, in the build path
build_target = "uking"
# ^ these files can be *versioned* if your project targets multiple versions.
# For example, the versioned file list path would be `data/<version>/file_list.yml`

# toolchain_bin_paths: paths where the `find_tool()` API looks for tools to call,
# such as nx2elf and llvm-objdump
toolchain_bin_paths = ["toolchain/bin"]

[check_mismatch.asm_differ]
source_directories = ["src", "lib"]
```

See [config.rs](../src/viking_core/src/config.rs) for the full config schema.

## Adding the Python package

> Note: the Python package is required even if you are using the Rust CLI, unless
> you only use Rust tools that never call Python.

### From source with `uv`

> Note: installing the Python package from source requires a Rust toolchain.

Run `uv add` with the `git+https` GitHub URL:
```shell
uv add git+https://github.com/Pistonight/viking-next
```

Then, edit `pyproject.toml` to pin a version (which can be a commit or tag):
```toml
[project]
dependencies = [
    "nx-decomp-tools-viking @ git+https://github.com/Pistonight/viking-next@<version>",
]
```

### From source with `pip`

> Note: installing the Python package from source requires a Rust toolchain.

> Note: virtual environment setup is not shown. If you use pip directly, we assume you know what you are doing.

If your project uses `pyproject.toml`, add the same line to `dependencies`
as shown in the `uv` section above, and run `pip install -e .`.

If you use `requirements.txt` instead, add the following (replace `<version>`
with the tag or commit you want to pin):
```
nx-decomp-tools-viking @ git+https://github.com/Pistonight/viking-next@<version>
```

Then install it with `pip` into the Python environment:
```shell
pip install -r requirements.txt
```

Or install it directly with `pip` into a Python environment:
```shell
pip install 'nx-decomp-tools-viking @ git+https://github.com/Pistonight/viking-next@<version>'
```

### From pre-built wheels with `uv`

Add the dependency and wheel index to `pyproject.toml`, and pin the package to it.
Marking the index as `explicit` means `uv` only uses it for packages
that are pinned to it, and the package will never be resolved from PyPI. Replace
`<version>` with whatever version you want to pin.

```toml
[project]
dependencies = [
    "nx-decomp-tools-viking==<version>"
]

[[tool.uv.index]]
name = "viking"
url = "https://pistonight.github.io/viking-next/wheels/"
explicit = true

[tool.uv.sources]
nx-decomp-tools-viking = { index = "viking" }
```

Then run `uv sync`. You will see an error about `asm-differ` not being added explicitly.
UV does not allow wheels to contain URL dependencies, so you need to also add it and keep
it synced to the same commit referrenced by `nx-decomp-tools-viking`, which will be
in the error message.

```diff
dependencies = [
    "nx-decomp-tools-viking==<version>",
+++ "asm-differ @ git+https://github.com/..." # whatever uv tells you to add
]
```

### From pre-built wheels with `pip`

> Note: `pip` does not support pinning a package to an index. It looks up
> packages from all configured indexes, and picks the best match regardless of
> where it comes from. Always pin an exact version to reduce the risk.

If your project uses `pyproject.toml`, add the package to `dependencies`:
```toml
[project]
dependencies = [
    "nx-decomp-tools-viking==<version>",
]
```

`pyproject.toml` has no standard way to specify an index for `pip`, so pass
the wheel index when installing:
```shell
pip install --extra-index-url https://pistonight.github.io/viking-next/wheels/ -e .
```

To avoid passing the option every time, you can set it in the
`PIP_EXTRA_INDEX_URL` environment variable, or in your
[pip config file](https://pip.pypa.io/en/stable/topics/configuration/):
```shell
pip config --site set global.extra-index-url https://pistonight.github.io/viking-next/wheels/
```

Or install the package with `pip`, adding the wheel index as an extra index
so other dependencies can still be installed from PyPI:
```shell
pip install --extra-index-url https://pistonight.github.io/viking-next/wheels/ 'nx-decomp-tools-viking==<version>'
```

If you use `requirements.txt` instead, put the index option at the top of the file:
```
--extra-index-url https://pistonight.github.io/viking-next/wheels/
nx-decomp-tools-viking==<version>
```

Then install it with `pip` into the Python environment:
```shell
pip install -r requirements.txt
```

## Calling the Python CLI

If you have decided to use the Python CLI instead of the Rust CLI, you are all done!

You can call the Python CLI like this:
```shell
# With `uv`
uv run viking COMMAND ARGS...
# With `python3` directly
python3 -m viking COMMAND ARGS...
# Within the Python environment
viking COMMAND ARGS...
```

## Using the Python API

For calling the CLI programmatically, running project setup, accessing the file list
from Python, etc., please refer to the [API Documentation for Python](https://pistonight.github.io/viking-next/python).

Example:

```python
from viking import core, ui, setup

# ui module: printing and formatting
ui.step("getting config")
# Get the config object (read from tools/config.toml)
config = core.get_config()
# get the path to the original ELF
original_elf = config.repo.original_elf()
# run some setup
setup.convert_nso_to_elf("my_main.nso", original_elf)
# discover a tool and print its path using note formatting
ui.note(core.find_tool("llvm-objdump"))
```

The `cli` module contains CLI entry points for calling the CLI tools programmatically:
```python
import viking.cli
viking.cli.main_argv(["check"])
# or: viking.cli.check([])
```

## Installing the Rust CLI

The Rust CLI is a single multi-call binary, `viking`. You can pass the command and
args, as in `viking COMMAND ARGS...`, or rename the binary to the name of the command
(such as `check`), and call it as `COMMAND ARGS...`.

### Using the Python API

The Python package has helpers to bootstrap the Rust CLI:

```python
import viking.setup

# Install from source (requires `cargo`, which comes with the Rust toolchain).
# By default, this installs the same version as the Python package
viking_bin = viking.setup.rust_cli_install_source(
    # This is the root for `cargo install`.
    # The binary will be built to `<cargo_root>/bin/viking`.
    # Tracking info is stored in the cargo root as well,
    # so re-installing the same git commit is a no-op
    cargo_root = "tools",
    # (optional) additional cargo flags
    cargo_flags = [],
    # (optional) override the repo and/or commit to install from
    # repo = "...",
    # commit = "...",
)

# Install the pre-built binary. This downloads and extracts the binary from the
# GitHub release to the path you pass in.
viking_bin = "tools/bin/viking"
viking.setup.rust_cli_install_pre_built(viking_bin)

# You can also hardlink the binary to the command names,
# so you can, for example, call `tools/check` instead of `tools/viking check`
viking.setup.rust_cli_create_hardlinks(viking_bin, ["tools/check", "tools/decompme"])
```

Please also read the "Point the Rust CLI to Python" section below.

### Build from source with Cargo

Run the following cargo command, replacing the `<>` placeholders.

```shell
cargo install viking \
  --git https://github.com/Pistonight/viking-next \
  --rev <version> \
  --root <cargo_root> \
  --features global-allocator,cmd-all
```

The binary will be built to `<cargo_root>/bin/viking`, and the Cargo metadata
will be saved to `<cargo_root>` as well (you can suppress this with `--no-track`,
but then you need some other way to avoid unnecessary reinstallation).

Please also read the "Point the Rust CLI to Python" section below.

### Pre-built binary from GitHub release

The release artifact is named `viking-<target_triple>-<version>.tar.gz`. You can either
download it manually, or fetch it with another tool such as `cargo-binstall`:

```shell
cargo binstall viking@<version> \
  --git https://github.com/Pistonight/viking-next \
  --root <cargo_root>
```

### Point the Rust CLI to Python

The Rust CLI finds the Python environment automatically if you use uv
(`uv` is in `PATH`) and `uv.lock` is in the same directory as either the config file
or the binary. No additional setup is required in this case!

Otherwise, you need to call the Rust CLI with additional environment variables.
See [`python::set_python_command`](https://pistonight.github.io/viking-next/rust/viking/python/fn.set_python_command.html).

## Using the Rust API

If you are writing other tools in Rust that want to reuse viking's functionality,
you can use its Rust API. See the [API Documentation for Rust](https://pistonight.github.io/viking-next/rust).

To add `viking` as a dependency, add the following to your `Cargo.toml`, replacing `<version>` with a commit or version tag:
```toml
[dependencies]
viking = { git = "https://github.com/Pistonight/viking-next", rev = "<version>" }
```
