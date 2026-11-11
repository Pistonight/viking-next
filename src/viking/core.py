"""APIs backed by Rust

This is a thin layer that exposes Python API that calls
into Rust with the global state
"""

import atexit
from pathlib import Path

from . import _core

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


def get_config() -> _core.Config:
    """Get the config object loaded from the project configuration file"""
    global _config
    if _config is None:
        # config is immutable once loaded
        _config = core_state.clone_config()
    return _config


# ======= command =======

PyCmd = _core.PyCmd
PyCmdHost = _core.PyCmdHost

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
