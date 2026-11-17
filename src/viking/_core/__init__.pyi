from . import _core_cmd_check as _core_cmd_check, _core_cmd_decompme as _core_cmd_decompme, _core_cmd_listsym as _core_cmd_listsym, _core_fl as _core_fl, _core_ui as _core_ui
from collections.abc import Sequence
from pathlib import Path
from typing import Any, Final, final

__all__ = ["Config", "ConfigCheckMismatch", "ConfigCheckMismatchAsmDiffer", "ConfigDecompMe", "ConfigDecompMeTarget", "ConfigRepo", "PyCmd", "State", "demangle_or_as_is", "find_tool", "fini_core_state", "init_core_state", "parse_json_command", "raw_or_demangled_contains", "_core_cmd_check", "_core_cmd_decompme", "_core_cmd_listsym", "_core_fl", "_core_ui"]

@final
class Config:
    """Structure of the TOML config file"""
    def __str__(self, /) -> str: ...
    @property
    def check_mismatch(self, /) -> ConfigCheckMismatch:
        """The `[check_mismatch]` section"""
    @property
    def decomp_me(self, /) -> ConfigDecompMe | None:
        """The `[decomp_me]` section

        Required to use the decompme tool
        """
    @property
    def repo(self, /) -> ConfigRepo:
        """The `[repo]` section"""

@final
class ConfigCheckMismatch:
    """Configuration for checking mismatches"""
    def __str__(self, /) -> str: ...
    @property
    def asm_differ(self, /) -> ConfigCheckMismatchAsmDiffer:
        """asm_differ options"""
    @property
    def check_unimplemented_references(self, /) -> bool:
        """Check function call targets even when the branch target is in the PLT
        section
        """
    @property
    def file_list_object_path_relative_to(self, /) -> list[str]:
        """Treat the object paths in the YAML file list as relative to one of
        these paths (relative to project root). If not specified or empty,
        the object paths should be treated as relative to the project root
        itself.

        This is useful to have to avoid changing the file list when refactoring the project
        in the future.

        For example, suppose a source file is `<root>/src/Foo/fooBar.cpp`,
        usually this would require the object path be `src/Foo/fooBar.o`.
        You can set this config to `["src/"]`, and the object path can be
        listed as `Foo/fooBar.o` in the file list.
        """
    @property
    def no_object_check_for(self, /) -> list[str]:
        """Skip the "object checks" for object names starting with these prefixes.

        "object checks" are checks other than the asm-matching check, such as
        placement, link order, NON_MATCHING comment...
        """

@final
class ConfigCheckMismatchAsmDiffer:
    """Options for calling `asm-differ`"""
    def __str__(self, /) -> str: ...
    @property
    def source_directories(self, /) -> list[str]:
        """`source_directories` config option in asm_differ; will be appended to repo.root_path"""

@final
class ConfigDecompMe:
    """Configuration for creating decomp.me snippets"""
    def __str__(self, /) -> str: ...
    @property
    def override_compile_flags(self, /) -> bool:
        """Toggle overriding of default flags with flags discovered in compilation database. (True by default)"""
    @property
    def target(self, /) -> ConfigDecompMeTarget:
        """Target configuration. Must specify either ( Compiler and Flags ) or ( Preset Id )."""

class ConfigDecompMeTarget:
    """```rust
    pub enum ConfigDecompMeTarget {
        Preset {
            /// Preset ID used for categorizing. Requires registering preset with compiler and flags.
            preset_id: String,
        },
        CompilerAndFlags {
            /// Name of the compiler used to compile the code
            compiler_name: String,
            /// Compilation flags that are used for creating scratches.
            default_compile_flags: String,
        },
    }
    ```
    """
    def __str__(self, /) -> str: ...
    @final
    class CompilerAndFlags(ConfigDecompMeTarget):
        """@private"""
        __match_args__: Final = ("compiler_name", "default_compile_flags")
        def __new__(cls, /, compiler_name: str, default_compile_flags: str) -> ConfigDecompMeTarget.CompilerAndFlags: ...
        @property
        def compiler_name(self, /) -> str: ...
        @property
        def default_compile_flags(self, /) -> str: ...
    @final
    class Preset(ConfigDecompMeTarget):
        """@private"""
        __match_args__: Final = ("preset_id",)
        def __new__(cls, /, preset_id: str) -> ConfigDecompMeTarget.Preset: ...
        @property
        def preset_id(self, /) -> str: ...

@final
class ConfigRepo:
    """Configuration for general project information"""
    def __str__(self, /) -> str: ...
    @property
    def address_base(self, /) -> int:
        """The address base used by data files and decompilation database"""
    def build_ninja(self, /) -> Path:
        """Get the path to the versionless build.ninja build file"""
    def build_ninja_with_version(self, /, version: str | None) -> Path:
        """Get the path to the optionally-versioned build.ninja build file"""
    @property
    def build_path(self, /) -> Path:
        """Path to the build directory, relative to `root_path`

        The build directory can be versioned:
        - `build/*` - build files for the blank version
        - `build/<version>/*` - build files for `<version>`
        """
    @property
    def build_target(self, /) -> str:
        """Name of the decomp elf inside the build or versioned build directory"""
    def compilation_database(self, /) -> Path:
        """Get the path to the versionless compilation database JSON file"""
    def compilation_database_with_version(self, /, version: str | None) -> Path:
        """Get the path to the optionally-versioned compilation database JSON file"""
    @property
    def data_path(self, /) -> Path:
        """Path to the data directory, relative to `root_path`

        The data directory can be versioned:
        - `data/*` - data files for the blank version
        - `data/<version>/*` - data files for `<version>`
        """
    def data_symbols(self, /) -> list[Path]:
        """Get the paths to the versionless data symbol list files."""
    def data_symbols_with_version(self, /, version: str | None) -> list[Path]:
        """Get the paths to the optionally-versioned data symbol list files."""
    def decomp_elf(self, /) -> Path:
        """Get the path to the versionless decomp elf"""
    def decomp_elf_with_version(self, /, version: str | None) -> Path:
        """Get the path to the optionally-versioned decomp elf"""
    @property
    def default_version(self, /) -> str | None:
        """Default version to use when selecting versioned data or build directory/file."""
    def file_list(self, /) -> Path:
        """Get the path to the versionless file list file."""
    def file_list_with_version(self, /, version: str | None) -> Path:
        """Get the path to the optionally-versioned file list file."""
    def original_elf(self, /) -> Path:
        """Get the path to the versionless original game elf"""
    def original_elf_with_version(self, /, version: str | None) -> Path:
        """Get the path to the optionally-versioned original game elf"""
    @property
    def root_path(self, /) -> Path:
        """Root directory of the repo. Relative to the containing directory
        of the config file. All other paths in the config are relative to
        the root path.
        """
    @property
    def toolchain_install_path(self, /) -> Path:
        """Path relative to root for setup to install toolchains"""

class PyCmd:
    """Command send from Rust to Python to execute some Python in the middle of Rust

    ```rust
    pub enum PyCmd {
        /// Run asm-differ with the given arguments
        AsmDiffer(Vec<String>),
        /// Print optional arguments of asm-differ
        AsmDifferOptionalArgsHelp(),
    }
    ```
    """
    def __str__(self, /) -> str: ...
    @final
    class AsmDiffer(PyCmd):
        """@private"""
        __match_args__: Final = ("_0",)
        @property
        def _0(self, /) -> list[str]: ...
        def __getitem__(self, /, key: int) -> Any: ...
        def __len__(self, /) -> int: ...
        def __new__(cls, /, _0: Sequence[str]) -> PyCmd.AsmDiffer: ...
    @final
    class AsmDifferOptionalArgsHelp(PyCmd):
        """@private"""
        __match_args__: Final = ()
        def __getitem__(self, /, key: int) -> Any: ...
        def __len__(self, /) -> int: ...
        def __new__(cls, /) -> PyCmd.AsmDifferOptionalArgsHelp: ...

@final
class State:
    """Global (per-module) state of the Rust module"""
    def clone_config(self, /) -> Config:
        """Clone the config into the Python side"""

def demangle_or_as_is(name: str) -> str:
    """Try demangle the name, or return the name unchanged if it cannot be demangled"""
def find_tool(state: State, tool: str) -> Path: ...
def fini_core_state(state: State) -> None:
    """Finalize the rust extension module"""
def init_core_state(python_command: Sequence[str]) -> State:
    """Initialize the rust extension module"""
def parse_json_command(s: str) -> PyCmd: ...
def raw_or_demangled_contains(function_name: str, query: str) -> bool:
    """Check if the function name contains `query` either as-is or when demangled.

    Returns `true` if:

    - `function_name` contains `query`, or
    - `demangle(function_name)` contains `query`
    """
