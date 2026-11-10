# viking

This is the rewrite and spiritual successor of [nx-decomp-tools](https://github.com/open-ead/nx-decomp-tools).

The goals of the rewrite are:
- **Deduplication**: Combine the Python and Rust tools into a single Python package
  with a Rust extension module. Functionality is either implemented in Python,
  or implemented in Rust and exposed as Python API. No duplication in Rust and Python.
- **Unification**: While the old repo was inactive, the SMO decomp community has forked
  with more bytecode checks and file list format change. This rewrite is a unified version
  that ports these improvements back with backward compatibility. 
- **Distribution**: The old tool required downstream projects to install it in a certain
  way and hard-coded many paths. The rewrite ships as a single Python package
  that can be installed with any Python package manager. The TOML config file makes it
  work with any project structure and has defaults that are reasonable and backward compatible.

## Installation

For decomp projects that use UV, uun the following command
```shell
uv add git+https://github.com/Pistonight/viking
```

Or edit `pyproject.toml` directly and optionally pin the revision
```toml
[project]
dependencies = [
    "viking",
]

[tool.uv.sources]
viking = { git = "https://github.com/Pistonight/viking-next", rev = "<commit hash>" }
```

For other types of package managers, please refer to their documentations
to install from a Git URL.

## Configuration

For backward compatibility, the default config location is `tools/config.toml`
relative to the root of your project repo. The tool searches parent directories
as well to discover the config, so you can run it from any subdirectory within the project.
Alternatively, use the `VIKING_CONFIG` environment variable to point to the exact
path of the config file (must be relative to current directory or absolute).

In the config file, `repo.root_path` should be a path relative to the containing
directory of the config file. All other paths are relative to the root path unless
otherwise specified.

See [config.rs](src/viking_core/src/config.rs) for the config schema, or
see [BOTW](https://github.com/zeldaret/botw/blob/master/tools/config.toml) for an example.

## Usage

### CLI usage

The Python package comes with a `viking` executable. Depending on how it's installed,
you might need to run it with the Python package manager or interpreter,
like `uv run viking` or `python -m viking`.

Run `viking COMMAND` to run a tool. Run `viking --help` to display the usage help.

### API usage and examples

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

The `viking._core` module contains generated Rust bindings and should NOT be used directly.


## Contributing
See [the contributing guidelines](./CONTRIBUTING.md)

## Credits
[leoetlino](https://github.com/leoetlino) and all contributors of
[nx-decomp-tools](https://github.com/open-ead/nx-decomp-tools)
