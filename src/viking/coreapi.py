import atexit

from . import _core

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
