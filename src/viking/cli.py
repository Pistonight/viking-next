"""nx-decomp-tools CLI

Usage: viking COMMAND [ARGS...]
"""

import sys
import typing

from ._core import cmd_check
from .asm_differ import asm_differ_optional_arguments_help
from .coreapi import core


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
    main_argv(sys.argv[1:])


def main_argv(args: list[str]) -> typing.NoReturn:
    """Main entrypoint. The args should not contain the executable"""
    if len(args) < 1:
        print_help()
        sys.exit(0)
    command_name = args[0]
    for command in _commands():
        if command_name == command.__name__:
            command(args[1:])
            # in case command returns because of bad implementation
            sys.exit(126)  # pyright: ignore[reportUnreachable]

    print(f"Unknown command: {command_name}")
    print_help()
    sys.exit(127)


def check(args: list[str]) -> typing.NoReturn:
    """Check compiled bytecode of a function matches the assembly found within the game"""
    parsed_args = cmd_check.parse_args(core, args)
    if parsed_args.print_help:
        cmd_check.print_help()
        print("asm-differ arguments:")
        print(asm_differ_optional_arguments_help())
        sys.exit(0)

    raise RuntimeError("not implemented")


def decompme(_args: list[str]) -> typing.NoReturn:
    """Upload a function to decomp.me"""
    raise RuntimeError("not implemented")


def listsym(_args: list[str]) -> typing.NoReturn:
    """Print symbols in the output ELF"""
    raise RuntimeError("not implemented")
