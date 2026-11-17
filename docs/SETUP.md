# Setup nx-decomp-tools-viking

> Note: the tools support Linux-x64 with incomplete support for Linux-arm64 and Macos-arm64.
> The ARM platforms should technically work but they aren't tested as much.

Before getting started, you need to decide on 2 things about how your project
would depend on this tool:

1. **Source vs. binary release**: Since this tool is very niche, instead of
   publishing it to well-known package registries like PyPI or crates.io, we
   host everything on GitHub, meaning you will need to point your tooling
   to GitHub (e.g. in `pyproject.toml`, `Cargo.toml`, etc). This repo provides
   both sources and pre-built binaries.
   - Install from *sources*:
     - Pros: Works nicely with most Python tools, like `uv` and the OG `pip`.
     - Cons: Requires Rust toolchain to install, and contributors to your project
       needs to compile the tools upon installation, which is a little bit slow.
   - Install from *pre-built binaries*:
     - Pros: Faster setup for your contributors. Does not require Rust.
     - Cons:
         - `pip` can install pre-built wheels from a GitHub-hosted link,
           but it will always prioritize PyPI, which is a potential supply-chain attack
           vector (i.e. if someone name-squats `nx-decomp-tools-viking`). `uv` does
           not have this issue.
         - The pre-built binaries target recent operating system and might not work
           on old computers.

2. **Python vs Rust CLI**: Most tools are written in Rust, wrapped with a Python
   API to make it easier to write scripts. For the CLI tools, you can both call
   them from the Python package (e.g. `uv run viking` or `python3 -m viking`),
   or call a native Rust binary directly.
     - Calling *Python CLI*:
       - Pros: No additional setup. Your existing workflows can call
         `uv run viking ...` or `python3 -m viking ...` directly, or
         through the Python CLI API programmatically.
       - Cons: There will be the overhead of calling the Python interpreter
         even for pure Rust tools. Especially for fast inner-loop workflows
         like checking match for a single function, the difference will be
         noticable if you are a performance-sensitive person.
     - Calling *Rust CLI*:
       - Pros: The native CLI only spawns a Python process if and when needed,
         which means it's a bit faster.
       - Cons: You will need a lot of additional setup, including
         installing the native binaries besides the Python package.
         The Python package is still required for tools that are implemented
         in Python (such as `asm-differ`). There also needs to be additional
         setup to allow the Rust CLI to call the Python CLI when needed.

## Dependencies

- Git: We have a few dependencies that are resolved from GitHub. You will
  need Git even if you are installing pre-built binaries.
- Python: It's recommended to use [`uv`](https://docs.astral.sh/uv/getting-started/installation/) as the project manager for Python,
  but any tool that can work with `pyproject.toml` should work (Python 3.14+). The Python
  version requirement is also specified there.
- Rust: *optional - only needed if you are installing Rust CLI from source*.
  The recommended way to setup Rust is through the official toolchain manager [Rustup](https://rustup.rs)
- `nx2elf`: Required for converting NSO to ELF.
- `llvm-objdump`: Required for disassembly diffing.

(`nx2elf` and `llvm-objdump` may be provided internally with the installation in the future).
  

## Configuration

The tool reads a single config file, which should be at:
- `tools/config.toml` in your repo, or
- set the `VIKING_CONFIG` environment variable to point to the exact location
  of the config file, either absolute or relative to the current working directory.

A minimal config file looks like this
```toml
[repo]
# root_path: relative to the containing directory of the config file,
# all other paths in the config file are relative to the root path
root_path = ".."
# data_path: directory that stores the original ELF, function/file list,
# as well as other data files that might be added in the future
data_path = "data"
# build_path: ideally a CMake build directory that contains output
# of your project
build_path = "build"

# file_list: name of the YAML file list, in the data path
file_list = "file_list.yml"
# build_target: name of the executable, in the build path
build_target = "uking"
# ^ these files can be *versioned*, if your project targets multiple versions.
# For example, the versioned file list path would be `data/<version>/file_list.yml`

# toolchain_bin_paths: paths for `find_tool()` API to find tools to call,
# such as nx2elf and llvm-objdump 
toolchain_bin_paths = ["toolchain/bin"]

[check_mismatch.asm_differ]
source_directories = ["src", "lib"]
```

See [config.rs](../src/viking_core/src/config.rs) for the full config schema.

## Adding the Python package

> Note: the Python package is required even if you are using Rust CLI, unless
> you are only using the Rust tools that will never call Python.

### From source with `uv`

> Note: installing the Python package from source will require a Rust toolchain.

Run `uv add` with the `git+https` GitHub URL:
```shell
uv add git+https://github.com/Pistonight/viking-next
```

Then, edit `pyproject.toml` to pin a version (which can be a commit or tag)
```toml
[project]
dependencies = [
    "nx-decomp-tools-viking @ git+https://github.com/Pistonight/viking-next@<version>",
]
```

### From source with `pip`

> Note: installing the Python package from source will require a Rust toolchain.

> Note: we are not showing the virtual environment setup. If using pip directly, we assume you know what you are doing.

If your project uses `pyproject.toml`, add the same line to `dependencies`
as shown in the `uv` section above, and run `pip install -e .`.

If you use `requirements.txt` instead, add the following (replace `<version>`
with the tag or commit you want to pin.)
```
nx-decomp-tools-viking @ git+https://github.com/Pistonight/viking-next@<version>
```

Then install it with `pip` into the Python environment
```shell
pip install -r requirements.txt
```

Or install it directly with `pip` into a Python environment
```shell
pip install 'nx-decomp-tools-viking @ git+https://github.com/Pistonight/viking-next@<version>'
```

### From pre-built wheels with `uv`

Add the wheel index to `pyproject.toml`, and pin the package to it.
Marking the index as `explicit` means `uv` only uses it for packages
that are pinned to it, and the package will never be resolved from PyPI.
```toml
[[tool.uv.index]]
name = "viking"
url = "https://pistonight.github.io/viking-next/wheels/"
explicit = true

[tool.uv.sources]
nx-decomp-tools-viking = { index = "viking" }
```

Then add the package, pinning a version:
```shell
uv add "nx-decomp-tools-viking==<version>"
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

Then install it with `pip` into the Python environment
```shell
pip install -r requirements.txt
```


## Calling the Python CLI

If you have decided to use the Python CLI instead of Rust CLI, you are all done now!

You can call the Python CLI like the following:
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
using Python, etc, please refer to the [API Documentation for Python](https://pistonight.github.io/viking-next/python)

Example:

```python
from viking import core, ui, setup

# ui module: printing and formatting
ui.step("getting config")
# Get the config object (read from tools/config.toml)
config = core.get_config()
# get path to the original ELF using note formatting
original_elf = config.repo.original_elf()
# run some setup
setup.convert_nso_to_elf("my_main.nso", original_elf)
# discover a tool and print its path
ui.note(core.find_tool("llvm-objdump"))
```

The `cli` module contains CLI entry points for calling CLI tools programmatically 
```python
import viking.cli
viking.cli.main_argv(["check"])
# or: viking.cli.check([])
```

## Installing the Rust CLI

**NOTE: This is not implemented yet!!**

The Rust CLI is a single multi-call binary `viking`. You can pass the command and
args, such as `viking COMMAND ARGS...`, or rename the binary to the name of the command
(such as `check`), and call it like `COMMAND ARGS...`.

### Using the Python API

The Python package has helpers to bootstrap the Rust CLI:

```python
import viking.setup

# Install from source (Requires `cargo`, which should come with the Rust toolchain)
# this will automatically install the same version
viking_bin = viking.setup.rust_cli_install_source(
    # This is the root for `cargo install`
    # The binary will be built to `<cargo_root>/bin/viking`
    # Tracking info will be stored in the cargo root as well,
    # so re-installing the same version that is already installed is a no-op
    cargo_root = "tools",
    # (optional) additional cargo flags
    cargo_flags = [],
)

# Install pre-built binary. This just downloads the binary from GitHub release
# to whatever path you pass in.
viking_bin = "tools/bin/viking"
viking.setup.rust_cli_install_prebuilt(viking_bin)

# If you want to hardlink the tools, you can do that as well,
# so you can for example call `tools/check` instead of `tools/viking check`
viking.setup.rust_cli_hardlinks(viking_bin, ["tools/check", "tools/decompme"])
```

Please also read the "Point the Rust CLI to Python" section below.

### Build from source with Cargo

Run the following cargo command, replace the `<>` placeholders with what you want.

```shell
cargo install viking \
  --git https://github.com/Pistonight/viking-next \
  --rev <version> \
  --root <cargo_root> \
  --features global-allocator,cmd-all
```

The binary will be built to `<cargo_root>/bin/viking` and the Cargo metadata
will be saved to `<cargo_root>` as well (you can suppress with `--no-track`,
but you would need some other way to avoid unnecessary reinstallation).

Please also read the "Point the Rust CLI to Python" section below.

### Pre-built binary from GitHub release

The release artifact is named `viking-<target_triple>.tar.gz`. You can either
download it manually, or fetch it with another tool such as `cargo-binstall`:

```shell
cargo binstall viking@<version> \
  --git https://github.com/Pistonight/viking-next \
  --root <cargo_root>
```

### Point the Rust CLI to Python

The Rust CLI will find the Python environment automatically
if you use UV and the binary is in the same directory as `uv.lock`,
no additional setup will be required in this case!

Otherwise, you need to call the Rust CLI with additional environment variables:
- `VIKING_PYTHON_COMMAND`: The command to call the Python CLI. For example
  `python3 -m viking` or `uv run viking`. Shell quoting is currently NOT supported (please
  open a PR if you require it).
- Other environment variables that are needed to call the Python project,
  for example `UV_PROJECT` if you use UV.


## Using the Rust API

If you are write other tools in Rust that want to reuse viking's functionality,
you can use its Rust API. See [API Documentation for Rust](https://pistonight.github.io/viking-next/rust)

To add `viking` as a dependency, add the following to your `Cargo.toml`, replacing `<version>` with a commit or version tag:
```toml
[dependencies]
viking = { git = "https://github.com/Pistonight/viking-next", rev = "<version>" }
```
