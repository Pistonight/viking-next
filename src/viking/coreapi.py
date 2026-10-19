import atexit
import traceback

from . import _core
from ._core import PyCmdHostHandle, PyCmd

core = _core.init_core_state()


@atexit.register
def _fint_core():
    _core.fini_core_state(core)


config = None


def get_config():
    """Get the config object loaded from the project configuration file"""
    global config
    if config is None:
        # config is immutable once loaded
        config = core.config()
    return config


def find_tool(tool: str):
    """Find a tool binary. Returns the absolute path"""
    return _core.find_tool(core, tool)

def run_cmd_host_loop(cmd_host: PyCmdHostHandle):
    while cmd := cmd_host.recv():
        match cmd:
            case PyCmd.AsmDiffer(args):
                # need this here to avoid circular import
                from .asm_differ import asm_differ
                try:
                    asm_differ(args)
                    cmd_host.send_result(None)
                except SystemExit as e:
                    if e.code == 0:
                        cmd_host.send_result(None)
                    else:
                        cmd_host.send_result(f"asm-differ exited with code {e.code}")
                except Exception:
                    traceback.print_exc()
                    cmd_host.send_result(f"an exception occured within asm-differ")
            case _:
                cmd_host.send_result("unexpected: python cmd_host received unknown command")
    cmd_host.join()
                       

