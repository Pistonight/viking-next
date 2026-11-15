__all__ = ["clear_terminal", "confirm", "format_address", "format_symbol_name", "print_error", "print_error_with_detail", "print_note", "print_step", "print_warning", "print_warning_with_detail"]

def clear_terminal() -> None:
    """Clear the terminal"""
def confirm(prompt: str, default_value: bool, help: str | None) -> None: ...
def format_address(addr: int) -> str:
    """Format an address for display"""
def format_symbol_name(name: str) -> str:
    """Format a mangled symbol name for display. Show as `{demangled} ({mangled})`"""
def print_error(msg: str) -> None:
    """Print a message using error formatting"""
def print_error_with_detail(msg: str, detail: str) -> None:
    """Print a message using error formatting plus details. The details
    are automatically wrapped.

    ```text
    error: {msg}
      |  detail...
      |  detail...
      |  detail...
    ```
    """
def print_note(msg: str) -> None:
    """Print a message using the note formatting"""
def print_step(msg: str) -> None:
    """Print a message using the step formatting"""
def print_warning(msg: str) -> None:
    """Print a message using warning formatting"""
def print_warning_with_detail(msg: str, detail: str) -> None:
    """Print a message using warning formatting plus details. The details
    are automatically wrapped.

    ```text
    warning: {msg}
      |  detail...
      |  detail...
      |  detail...
    ```
    """
