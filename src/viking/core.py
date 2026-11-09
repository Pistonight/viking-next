"""APIs backed by Rust

This is a thin layer that exposes Python API that calls
into Rust with the global state
"""

import atexit
import traceback

from . import _core

# While we could use process-wide static singleton in Rust,
# it will not play nicely going forward with Python's
# multi-interpreter. See pyapi.rs for more information

# Any Rust code that needs this global state should
# take core_state (StateHandle in Rust) as the first parameter
# of the _core.* API

core_state = _core.init_core_state()


@atexit.register
def _fint_core():
    _core.fini_core_state(core_state)


_config = None


def get_config():
    """Get the config object loaded from the project configuration file"""
    global _config
    if _config is None:
        # config is immutable once loaded
        _config = core_state.config()
    return _config


def find_tool(tool: str):
    """Find a tool binary. Returns the absolute path"""
    return _core.find_tool(core_state, tool)


def run_cmd_host_loop(cmd_host: _core.PyCmdHostHandle):
    while cmd := cmd_host.recv():
        match cmd:
            case _core.PyCmd.AsmDiffer(args):
                # need this here to avoid circular import
                from .asm_differ import asm_differ

                try:
                    print("launching asm-differ...")
                    asm_differ(args)
                    cmd_host.send_result(None)
                except SystemExit as e:
                    if e.code == 0:
                        cmd_host.send_result(None)
                    else:
                        cmd_host.send_result(f"asm-differ exited with code {e.code}")
                except Exception:  # noqa: BLE001
                    traceback.print_exc()
                    cmd_host.send_result("an exception occured within asm-differ")
            case _:
                cmd_host.send_result(
                    "unexpected: python cmd_host received unknown command"
                )
    cmd_host.join()
