import sys
from pathlib import Path

# preload diff_settings which will let asm_differ always
# find the settings supplied by us. Downstream projects
# should configure them through config.toml
sys.path.insert(0, str(Path(__file__).resolve().parent / "env"))
import diff_settings  # pyright: ignore[reportMissingImports]

_ = diff_settings  # mark as used

from diff import main_early, main_late  # pyright: ignore[reportMissingTypeStubs]

# this initializes the global args parser used by asm_differ
main_early()

# hijack the parser so we can inject args, instead of parsing from the command line args
from diff import parser as _asm_differ_cli_parser  # pyright: ignore[reportMissingTypeStubs]

assert _asm_differ_cli_parser is not None, "set by main_early"
# override its program name
_asm_differ_cli_parser.prog = "asm-differ"


def asm_differ(args: list[str]):
    """Invoke asm_differ's diff.py script with the args"""

    class FakeParser:
        def parse_args(self):
            return _asm_differ_cli_parser.parse_args(args)

    import diff  # pyright: ignore[reportMissingTypeStubs]

    diff.parser = FakeParser()
    main_late()
    diff.parser = _asm_differ_cli_parser


def asm_differ_optional_arguments_help() -> str:
    help = _asm_differ_cli_parser.format_help()
    parts = help.split("optional arguments:")
    if len(parts) < 2:
        parts = help.split("options:")

    def remove_help(s: str) -> str:
        return "\n".join(x for x in s.splitlines() if "--help" not in x)

    if len(parts) < 2:
        return remove_help(help)
    return remove_help(parts[1])
