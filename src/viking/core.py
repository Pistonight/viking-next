"""APIs backed by Rust

This is a thin layer that exposes Python API that calls
into Rust with the global state
"""

import atexit

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
