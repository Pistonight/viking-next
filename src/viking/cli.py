"""nx-decomp-tools CLI

Usage: viking COMMAND [ARGS...]
"""

# This is the CLI entry point
#
# To invoke programmatically from python:
# ```
# import viking.cli
# viking.cli.main_argv(["check", "MyFunction"])
# # or use each tool directly
# viking.cli.check(["MyFunction"])
# ```

import sys
import traceback
import typing

from ._core import _core_cmd_check, _core_cmd_decompme, _core_cmd_listsym
from .core import core_state, run_cmd_host_loop
from .asm_differ import asm_differ_optional_arguments_help


def _commands():
    return [check, decompme, listsym]


def print_help():
    print(__doc__)
    print("Commands:")
    for command in _commands():
        print(f"  {command.__name__:<12}{command.__doc__}")
    print()
    print("Run viking COMMAND --help to see help for each command")


def main() -> typing.NoReturn:
    """Main entrypoint parsing args from sys.argv"""
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
    parsed_args = _core_cmd_check.parse_args(core_state, args)
    if parsed_args.print_help:
        _core_cmd_check.print_help()
        print("asm-differ arguments:")
        print(asm_differ_optional_arguments_help())
        sys.exit(0)

    cmd_host = _core_cmd_check.run(core_state, parsed_args)
    run_cmd_host_loop(cmd_host)
    sys.exit(0)


def decompme(args: list[str]) -> typing.NoReturn:
    """Upload a function to decomp.me"""
    exit_status = _core_cmd_decompme.run(core_state, args)
    sys.exit(exit_status)


def listsym(args: list[str]) -> typing.NoReturn:
    """Print symbols in the output ELF"""
    exit_status = _core_cmd_listsym.run(core_state, args)
    sys.exit(exit_status)
