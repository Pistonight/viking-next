"""
CLI entry points

### Examples
```python
import viking.cli

viking.cli.main_argv(["check", "MyFunction"])

# or use each tool directly
viking.cli.check(["MyFunction"])
```
"""

import sys
import typing


def _commands():
    return [check, decompme, listsym, py_json_call]


def print_help():
    """Print help text, which shows what commands are available"""

    print("NX decomp tools CLI\n\nUsage: viking COMMAND [ARGS...]\n")
    print("Commands:")
    for command in _commands():
        if not command.__doc__ or command.__doc__.startswith("@private"):
            continue
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
        import traceback

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
    command_name_orig = args[0]
    command_name = command_name_orig.replace("-", "_")
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

    if command_name_orig != "-h" and command_name_orig != "--help":
        print(f"Unknown command: {command_name_orig}")
    print_help()
    sys.exit(127)


def py_json_call(args: list[str]) -> typing.NoReturn:
    """@private Execute a json-serialized command"""
    # In this path we don't import .core to skip config loading
    # which saves point something ms
    if len(args) == 0:
        from . import ui

        ui.error("expected serialized json command")
        sys.exit(127)
    from . import _core
    from .core import run_json_command

    run_json_command(_core.parse_json_command(args[0]))


def check(args: list[str]) -> typing.NoReturn:
    """Check compiled bytecode of a function matches the assembly found within the game"""
    from ._core import _core_cmd_check
    from .core import core_state

    exit_status = _core_cmd_check.run(core_state, args)
    sys.exit(exit_status)


def decompme(args: list[str]) -> typing.NoReturn:
    """Upload a function to decomp.me"""
    from ._core import _core_cmd_decompme
    from .core import core_state

    exit_status = _core_cmd_decompme.run(core_state, args)
    sys.exit(exit_status)


def listsym(args: list[str]) -> typing.NoReturn:
    """Print symbols in the output ELF"""
    from ._core import _core_cmd_listsym
    from .core import core_state

    exit_status = _core_cmd_listsym.run(core_state, args)
    sys.exit(exit_status)
