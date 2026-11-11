"""Core functionality such as loading the configuration file and Python-rust interop."""

import atexit
import traceback
from pathlib import Path

from . import _core, ui

# ======= module =======

# While we could use process-wide static singleton in Rust,
# it will not play nicely going forward with Python's
# multi-interpreter. See pyapi.rs for more information

# Any Rust code that needs this global state should
# take core_state (State in Rust) as the first parameter

core_state = _core.init_core_state()
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


def run_cmd_host_loop(cmd_host: _core.PyCmdHost):
    """@private Receive commands from Rust function running on a separate thread"""
    while (cmd := cmd_host.recv()) is not None:
        try:
            match cmd:
                case _core.PyCmd.AsmDiffer(args):
                    try:
                        # need this here to avoid circular import
                        from .asm_differ import asm_differ

                        ui.step("launching asm-differ...")
                        asm_differ(args)
                        cmd_host.send_result(True, "")
                    except SystemExit as e:
                        if e.code == 0:
                            cmd_host.send_result(True, "")
                        else:
                            cmd_host.send_result(
                                False, f"asm-differ exited with code {e.code}"
                            )
                    except KeyboardInterrupt:
                        # treat the same as exiting normally
                        cmd_host.send_result(True, "")
                case _core.PyCmd.AsmDifferOptionalArgsHelp():
                    # need this here to avoid circular import
                    from .asm_differ import asm_differ_optional_arguments_help

                    cmd_host.send_result(True, asm_differ_optional_arguments_help())
                case _:
                    cmd_host.send_result(
                        False, f"unexpected: did not implement handler for {cmd}"
                    )
        except Exception:  # noqa: BLE001
            traceback.print_exc()
            cmd_host.send_result(
                False, f"an exception occured while running the command: {cmd}"
            )

    cmd_host.join()


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
