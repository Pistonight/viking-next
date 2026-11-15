from . import State
from collections.abc import Sequence

__all__ = ["run"]

def run(state: State, args: Sequence[str]) -> int: ...
