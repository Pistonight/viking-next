"""
CLI entry points

## Examples
```python
import viking.cli

viking.cli.main_argv(["check", "MyFunction"])

# or use each tool directly
viking.cli.check(["MyFunction"])
```

Usage: viking COMMAND [ARGS...]
"""

USAGE = """NX decomp tools CLI

Usage: viking COMMAND [ARGS...]
"""

import sys
import traceback
import typing

from . import interop
from ._core import _core_cmd_check, _core_cmd_decompme, _core_cmd_listsym
from .core import core_state


def _commands():
    return [check, decompme, listsym]


def print_help():
    """Print help text, which shows what commands are available"""
    print(__doc__)
    print("Commands:")
    for command in _commands():
        print(f"  {command.__name__:<12}{command.__doc__}")
    print()
    print("Run viking COMMAND --help to see help for each command")


def main() -> typing.NoReturn:
    """Main entrypoint parsing args from `sys.argv`"""
    try:
        main_argv(sys.argv[1:])
    except SystemExit as e:
        sys.exit(e.code)
    except Exception as e:  # noqa: BLE001
        # hide this frame
        if e.__traceback__:
            e.__traceback__ = e.__traceback__.tb_next
        # colorize=True is an internal argument
        traceback.print_exception(e, colorize=True)  # pyright: ignore[reportCallIssue]
        sys.exit(1)


def main_argv(args: list[str]) -> typing.NoReturn:
    """Main entrypoint. The args should not contain the executable"""
    if len(args) < 1:
        print_help()
        sys.exit(0)
    command_name = args[0]
    for command in _commands():
        if command_name == command.__name__:
            try:
                command(args[1:])
            except Exception as e:
                # hide this frame
                if e.__traceback__:
                    e.__traceback__ = e.__traceback__.tb_next
                raise
            # in case command returns because of bad implementation
            sys.exit(126)  # pyright: ignore[reportUnreachable]

    print(f"Unknown command: {command_name}")
    print_help()
    sys.exit(127)


def check(args: list[str]) -> typing.NoReturn:
    """Check compiled bytecode of a function matches the assembly found within the game"""
    check_cmd = _core_cmd_check.run(core_state, args)
    exit_status = interop.run_cmd_host_loop(check_cmd)
    sys.exit(exit_status)


def decompme(args: list[str]) -> typing.NoReturn:
    """Upload a function to decomp.me"""
    exit_status = _core_cmd_decompme.run(core_state, args)
    sys.exit(exit_status)


def listsym(args: list[str]) -> typing.NoReturn:
    """Print symbols in the output ELF"""
    exit_status = _core_cmd_listsym.run(core_state, args)
    sys.exit(exit_status)
