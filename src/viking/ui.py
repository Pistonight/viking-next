import sys
import typing

from ._core import _core_ui

clear_terminal = _core_ui.clear_terminal
format_symbol_name = _core_ui.format_symbol_name
format_address = _core_ui.format_symbol_name


def confirm(prompt: typing.Any, default_value: bool = True, help: str | None = None):  # pyright: ignore[reportAny]
    """Show confirmation prompt. Raise exception on cancel

    ```text
    {prompt} (Y/n)
    [{help}]
    ```
    """
    _core_ui.confirm(str(prompt), default_value, help)  # pyright: ignore[reportAny]


def step(msg: typing.Any):  # pyright: ignore[reportAny]
    """Print a message using step formatting"""
    _core_ui.print_step(str(msg))  # pyright: ignore[reportAny]


def note(msg: typing.Any):  # pyright: ignore[reportAny]
    """Print a message using note formatting"""
    _core_ui.print_note(str(msg))  # pyright: ignore[reportAny]


def warn(msg: typing.Any):  # pyright: ignore[reportAny]
    """Print a message using warning formatting"""
    _core_ui.print_warning(str(msg))  # pyright: ignore[reportAny]


def warn_detail(msg: typing.Any, detail: typing.Any):  # pyright: ignore[reportAny]
    """Print a message using warning formatting plus details. The details are automatically wrapped.

    ```text
    warning: {msg}
      |  detail...
      |  detail...
      |  detail...
    ```
    """
    _core_ui.print_warning_with_detail(str(msg), str(detail))  # pyright: ignore[reportAny]


def bail(msg: typing.Any) -> typing.NoReturn:  # pyright: ignore[reportAny]
    """Exit with an error message"""
    error(msg)
    sys.exit(1)


def error(msg: typing.Any):  # pyright: ignore[reportAny]
    """Print a message using error formatting"""
    _core_ui.print_error(str(msg))  # pyright: ignore[reportAny]


def error_detail(msg: typing.Any, detail: typing.Any):  # pyright: ignore[reportAny]
    """Print a message using error formatting plus details. The details are automatically wrapped.

    ```text
    error: {msg}
    |  detail...
    |  detail...
    |  detail...
    ```
    """
    _core_ui.print_error_with_detail(str(msg), str(detail))  # pyright: ignore[reportAny]
