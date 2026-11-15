"""Core functionality such as loading the configuration file and Python-rust interop."""

import atexit
import sys
import typing
from pathlib import Path

from . import _core, ui

# ======= module =======

# While we could use process-wide static singleton in Rust,
# it will not play nicely going forward with Python's
# multi-interpreter. See pyapi.rs for more information

# Any Rust code that needs this global state should
# take core_state (State in Rust) as the first parameter


def _init_core_state():
    return _core.init_core_state([sys.executable, sys.argv[0]])


core_state = _init_core_state()
"""@private"""


@atexit.register
def _fint_core():
    _core.fini_core_state(core_state)


_config = None


def get_config() -> Config:
    """Get the config object loaded from the project configuration file"""
    global _config
    if _config is None:
        # config is immutable once loaded
        _config = core_state.clone_config()
    return _config


# ======= command =======


def run_json_command(cmd: _core.PyCmd) -> typing.NoReturn:
    """@private Execute command received from Rust"""
    match cmd:
        case _core.PyCmd.AsmDiffer(args):
            try:
                # need this here to avoid circular import
                from .asm_differ import asm_differ

                ui.step("launching asm-differ...")
                asm_differ(args)
            except KeyboardInterrupt:
                # treat the same as exiting normally
                pass
            sys.exit(0)
        case _core.PyCmd.AsmDifferOptionalArgsHelp():
            # need this here to avoid circular import
            from .asm_differ import asm_differ_optional_arguments_help

            print(asm_differ_optional_arguments_help())
            sys.exit(0)
        case _:
            raise Exception(f"unknown json command: {cmd}")  # noqa: TRY002


# ======= config =======

# type re-exports
Config = _core.Config
ConfigRepo = _core.ConfigRepo
ConfigCheckMismatch = _core.ConfigCheckMismatch
ConfigCheckMismatchAsmDiffer = _core.ConfigCheckMismatchAsmDiffer
ConfigDecompMe = _core.ConfigDecompMe
ConfigDecompMeTarget = _core.ConfigDecompMeTarget


def find_tool(tool: str) -> Path:
    """Find a tool binary. Returns the absolute path"""
    return _core.find_tool(core_state, tool)


demangle = _core.demangle_or_as_is
raw_or_demangled_contains = _core.raw_or_demangled_contains
