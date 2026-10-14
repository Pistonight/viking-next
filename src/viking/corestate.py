import atexit

from viking import _core

core = _core.init_core_state()


@atexit.register
def _fint_core():
    _core.fini_core_state(core)
