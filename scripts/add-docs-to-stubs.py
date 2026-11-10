"""
Dead simple doc string extractor for the generated stubs.

Scans STUBS_ROOT for .pyi files. For each class/function it finds,
it imports the object from the compiled extension module, reads its __doc__,
and writes it back into the .pyi file as a docstring.
"""

import ast
import importlib
import inspect
import types
import typing
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STUBS_ROOT = ROOT / "src" / "viking" / "_core"
# directory that the dotted module paths are relative to
PACKAGE_ROOT = ROOT / "src"

INDENT = "    "


@dataclass
class PyiEntity:
    # attribute path inside the module, e.g. ("Status", "from_code")
    attr_path: tuple[str, ...]
    # byte range in the original file to replace
    start: int
    end: int
    # indentation of the docstring line
    indent: str
    # text surrounding the docstring literal in the replacement
    prefix: str
    suffix: str


@dataclass
class PyiFile:
    path: Path
    # module path, e.g. ("viking", "_core", "_core_fl")
    module_path: tuple[str, ...]
    entities: list[PyiEntity]


def main() -> None:
    files = parse_pyi_files(STUBS_ROOT)
    for file in files:
        count = write_docs(file)
        print(f"{file.path.relative_to(ROOT)}: added {count} doc string(s)")


# --- step 1: parse --------------------------------------------------------


def parse_pyi_files(root: Path) -> list[PyiFile]:
    return [parse_pyi_file(path) for path in sorted(root.rglob("*.pyi"))]


def parse_pyi_file(path: Path) -> PyiFile:
    source = path.read_bytes()
    tree = ast.parse(source, filename=str(path))
    line_offsets = compute_line_offsets(source)
    entities: list[PyiEntity] = []
    collect_entities(tree.body, (), source, line_offsets, entities)
    return PyiFile(path=path, module_path=module_path_of(path), entities=entities)


def module_path_of(path: Path) -> tuple[str, ...]:
    parts = path.relative_to(PACKAGE_ROOT).with_suffix("").parts
    if parts[-1] == "__init__":
        parts = parts[:-1]
    return parts


def compute_line_offsets(source: bytes) -> list[int]:
    """Byte offset of the start of each line (index 0 is line 1)"""
    offsets = [0]
    for line in source.splitlines(keepends=True):
        offsets.append(offsets[-1] + len(line))
    return offsets


def collect_entities(
    body: list[ast.stmt],
    parent: tuple[str, ...],
    source: bytes,
    line_offsets: list[int],
    out: list[PyiEntity],
) -> None:
    for node in body:
        if isinstance(node, ast.ClassDef):
            attr_path = (*parent, node.name)
            out.append(make_entity(node, attr_path, source, line_offsets))
            collect_entities(node.body, attr_path, source, line_offsets, out)
        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            # dunder methods only have the generic docs from object
            if node.name.startswith("__") and node.name.endswith("__"):
                continue
            out.append(make_entity(node, (*parent, node.name), source, line_offsets))


def make_entity(
    node: ast.ClassDef | ast.FunctionDef | ast.AsyncFunctionDef,
    attr_path: tuple[str, ...],
    source: bytes,
    line_offsets: list[int],
) -> PyiEntity:
    def offset(lineno: int, col: int) -> int:
        # ast column offsets are in UTF-8 bytes
        return line_offsets[lineno - 1] + col

    first = node.body[0]
    assert first.end_lineno is not None and first.end_col_offset is not None
    start = offset(first.lineno, first.col_offset)
    end = offset(first.end_lineno, first.end_col_offset)

    if is_docstring(first):
        # already has a docstring, replace it
        indent = " " * first.col_offset
        return PyiEntity(attr_path, start, end, indent, prefix="", suffix="")

    if len(node.body) == 1 and is_ellipsis(first):
        # `def f(): ...` -> replace the `...` with the docstring on its own line
        # (also eat the space before `...` so no trailing whitespace is left)
        while source[start - 1 : start] == b" ":
            start -= 1
        indent = " " * node.col_offset + INDENT
        return PyiEntity(attr_path, start, end, indent, prefix="\n" + indent, suffix="")

    # insert before the first statement of the body
    indent = " " * first.col_offset
    line_start = line_offsets[first.lineno - 1]
    return PyiEntity(attr_path, line_start, line_start, indent, prefix=indent, suffix="\n")


def is_docstring(node: ast.stmt) -> bool:
    return (
        isinstance(node, ast.Expr)
        and isinstance(node.value, ast.Constant)
        and isinstance(node.value.value, str)
    )


def is_ellipsis(node: ast.stmt) -> bool:
    return (
        isinstance(node, ast.Expr)
        and isinstance(node.value, ast.Constant)
        and node.value.value is Ellipsis
    )


# --- step 2: read docs and write back ---------------------------------------


def write_docs(file: PyiFile) -> int:
    original = file.path.read_bytes()
    module = import_module(file.module_path)
    replacements: list[tuple[int, int, bytes]] = []
    for entity in file.entities:
        doc = read_doc(module, entity.attr_path)
        if not doc:
            continue
        text = entity.prefix + format_docstring(doc, entity.indent) + entity.suffix
        replacements.append((entity.start, entity.end, text.encode()))
    updated = apply_replacements(original, replacements)
    if updated != original:
        file.path.write_bytes(updated)
    return len(replacements)


def import_module(module_path: tuple[str, ...]) -> types.ModuleType:
    """
    Import the module. Submodules of a native extension are not importable
    with the import statement, so import the longest importable prefix
    and getattr the rest
    """
    for i in range(len(module_path), 0, -1):
        try:
            obj = importlib.import_module(".".join(module_path[:i]))
        except ModuleNotFoundError:
            continue
        for name in module_path[i:]:
            sub: object = getattr(obj, name)  # pyright: ignore[reportAny]
            if not isinstance(sub, types.ModuleType):
                raise TypeError(f"{obj.__name__}.{name} is not a module")
            obj = sub
        return obj
    raise ModuleNotFoundError(".".join(module_path))


def read_doc(module: types.ModuleType, attr_path: tuple[str, ...]) -> str | None:
    obj: object = module
    for name in attr_path:
        obj = typing.cast(object, inspect.getattr_static(obj, name, None))
        if obj is None:
            print(f"warning: cannot find {module.__name__}.{'.'.join(attr_path)}")
            return None
    if isinstance(obj, (staticmethod, classmethod)):
        obj = typing.cast(object, obj.__func__)
    doc = typing.cast(object, getattr(obj, "__doc__", None))
    if not isinstance(doc, str):
        return None
    return inspect.cleandoc(doc) or None


def format_docstring(doc: str, indent: str) -> str:
    doc = doc.replace("\\", "\\\\").replace('"""', '\\"\\"\\"')
    if doc.endswith('"'):
        doc = doc[:-1] + '\\"'
    lines = doc.splitlines()
    if len(lines) == 1:
        return f'"""{lines[0]}"""'
    body = "\n".join(indent + line if line else "" for line in lines[1:])
    return f'"""{lines[0]}\n{body}\n{indent}"""'


def apply_replacements(original: bytes, replacements: list[tuple[int, int, bytes]]) -> bytes:
    # apply from the end so earlier offsets stay valid
    result = original
    for start, end, text in sorted(replacements, key=lambda r: r[0], reverse=True):
        result = result[:start] + text + result[end:]
    return result


if __name__ == "__main__":
    main()
