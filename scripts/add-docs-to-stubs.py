"""
Dead simple doc string extractor for the generated stubs.

Scans STUBS_ROOT for .pyi files. For each class/function it finds,
it imports the object from the compiled extension module, reads its __doc__,
and writes it back into the .pyi file as a docstring.

Enums (#[pyclass] enums in Rust) are documented with their Rust source:
the `pub enum ... {}` block is found in RUST_SRC and appended to the enum's
docstring as a ```rust code block. The variants (nested classes for complex
enums, class attributes for simple enums) are marked with `@private` so pdoc
hides them, since the Rust source already documents them.
"""

import ast
import importlib
import inspect
import re
import textwrap
import types
import typing
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

ROOT = Path(__file__).resolve().parent.parent
STUBS_ROOT = ROOT / "src" / "viking" / "_core"
# directory that the dotted module paths are relative to
PACKAGE_ROOT = ROOT / "src"
RUST_SRC = ROOT / "src" / "viking_core" / "src"

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
    # enum: docstring gets the Rust source; variant: hidden with @private
    kind: Literal["normal", "enum", "variant"] = "normal"


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
    for i, node in enumerate(body):
        if isinstance(node, ast.ClassDef):
            attr_path = (*parent, node.name)
            entity = make_entity(node, attr_path, source, line_offsets)
            if parent and is_variant_class(node, parent[-1]):
                entity.kind = "variant"
            elif is_enum(node):
                entity.kind = "enum"
            out.append(entity)
            collect_entities(node.body, attr_path, source, line_offsets, out)
        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            # dunder methods only have the generic docs from object
            if node.name.startswith("__") and node.name.endswith("__"):
                continue
            out.append(make_entity(node, (*parent, node.name), source, line_offsets))
        elif (
            parent
            and isinstance(node, ast.AnnAssign)
            and isinstance(node.target, ast.Name)
            and is_variant_annotation(node.annotation, parent[-1])
        ):
            next_node = body[i + 1] if i + 1 < len(body) else None
            out.append(
                make_variant_variable_entity(
                    node, (*parent, node.target.id), next_node, line_offsets
                )
            )


def is_enum(node: ast.ClassDef) -> bool:
    """Check if the class is a PyO3 enum, by checking if it has any variant"""
    for child in node.body:
        if isinstance(child, ast.ClassDef) and is_variant_class(child, node.name):
            return True
        if isinstance(child, ast.AnnAssign) and is_variant_annotation(
            child.annotation, node.name
        ):
            return True
    return False


def is_variant_class(node: ast.ClassDef, enum_name: str) -> bool:
    """Complex enum variant: `class Variant(Enum): ...` nested in `class Enum`"""
    return any(
        isinstance(base, ast.Name) and base.id == enum_name for base in node.bases
    )


def is_variant_annotation(annotation: ast.expr, enum_name: str) -> bool:
    """Simple enum variant: `Variant: Final[Enum]` in `class Enum`"""
    if (
        isinstance(annotation, ast.Subscript)
        and isinstance(annotation.value, ast.Name)
        and annotation.value.id == "Final"
    ):
        annotation = annotation.slice
    return isinstance(annotation, ast.Name) and annotation.id == enum_name


def make_variant_variable_entity(
    node: ast.AnnAssign,
    attr_path: tuple[str, ...],
    next_node: ast.stmt | None,
    line_offsets: list[int],
) -> PyiEntity:
    """Docstring for a variable goes in the statement after it"""
    indent = " " * node.col_offset
    if next_node is not None and is_docstring(next_node):
        assert next_node.end_lineno is not None and next_node.end_col_offset is not None
        start = line_offsets[next_node.lineno - 1] + next_node.col_offset
        end = line_offsets[next_node.end_lineno - 1] + next_node.end_col_offset
        return PyiEntity(
            attr_path, start, end, indent, prefix="", suffix="", kind="variant"
        )
    assert node.end_lineno is not None and node.end_col_offset is not None
    end = line_offsets[node.end_lineno - 1] + node.end_col_offset
    return PyiEntity(
        attr_path, end, end, indent, prefix="\n" + indent, suffix="", kind="variant"
    )


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
    return PyiEntity(
        attr_path, line_start, line_start, indent, prefix=indent, suffix="\n"
    )


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
        if entity.kind == "variant":
            # documented by the Rust source in the enum's docstring
            doc = "@private"
        else:
            doc = read_doc(module, entity.attr_path)
            if entity.kind == "enum":
                doc = add_rust_source(entity.attr_path, doc)
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


# --- Rust enum source ------------------------------------------------------


@dataclass
class RustEnum:
    name: str
    # cleaned doc comment, None if not documented
    doc: str | None
    # the `pub enum ... {}` block, unindented
    source: str
    location: str


RUST_ENUM_RE = re.compile(r"^\s*pub enum (\w+)\b")
_rust_enums: dict[str, list[RustEnum]] | None = None


def add_rust_source(attr_path: tuple[str, ...], doc: str | None) -> str | None:
    """Append the Rust source of the enum to the doc as a code block"""
    rust_enum = find_rust_enum(attr_path[-1], doc)
    if rust_enum is None:
        return doc
    code_block = f"```rust\n{rust_enum.source}\n```"
    return f"{doc}\n\n{code_block}" if doc else code_block


def find_rust_enum(name: str, doc: str | None) -> RustEnum | None:
    """
    Find the #[pyclass] enum by name, using the doc comment to disambiguate
    (undocumented enums only match undocumented Rust enums)
    """
    global _rust_enums
    if _rust_enums is None:
        _rust_enums = index_rust_enums(RUST_SRC)
    candidates = [e for e in _rust_enums.get(name, []) if e.doc == doc]
    if len(candidates) != 1:
        found = ", ".join(e.location for e in _rust_enums.get(name, [])) or "none"
        print(
            f"warning: expected 1 #[pyclass] enum {name} with matching doc in Rust source, found {len(candidates)} (candidates: {found})"
        )
        return None
    return candidates[0]


def index_rust_enums(root: Path) -> dict[str, list[RustEnum]]:
    """Find all #[pyclass] `pub enum`s in the Rust source"""
    enums: dict[str, list[RustEnum]] = {}
    for path in sorted(root.rglob("*.rs")):
        lines = path.read_text().splitlines()
        for i, line in enumerate(lines):
            match = RUST_ENUM_RE.match(line)
            if not match:
                continue
            # walk up through the doc comments and attributes
            doc_lines: list[str] = []
            attrs: list[str] = []
            j = i - 1
            while j >= 0:
                stripped = lines[j].strip()
                if stripped.startswith("///"):
                    doc_lines.append(stripped.removeprefix("///"))
                elif stripped.startswith("#["):
                    attrs.append(stripped)
                else:
                    break
                j -= 1
            if not any("pyclass" in attr for attr in attrs):
                continue
            doc = inspect.cleandoc("\n".join(reversed(doc_lines))) or None
            name = match.group(1)
            location = f"{path.relative_to(ROOT)}:{i + 1}"
            source = extract_braced_block(lines, i, location)
            enums.setdefault(name, []).append(RustEnum(name, doc, source, location))
    return enums


def extract_braced_block(lines: list[str], start: int, location: str) -> str:
    """Extract lines from start until the matching closing brace, unindented"""
    depth = 0
    opened = False
    for end in range(start, len(lines)):
        # ignore braces in comments
        code = lines[end].split("//", 1)[0]
        depth += code.count("{") - code.count("}")
        opened = opened or "{" in code
        if opened and depth == 0:
            return textwrap.dedent("\n".join(lines[start : end + 1]))
    raise ValueError(f"unterminated enum at {location}")


def format_docstring(doc: str, indent: str) -> str:
    doc = doc.replace("\\", "\\\\").replace('"""', '\\"\\"\\"')
    if doc.endswith('"'):
        doc = doc[:-1] + '\\"'
    lines = doc.splitlines()
    if len(lines) == 1:
        return f'"""{lines[0]}"""'
    body = "\n".join(indent + line if line else "" for line in lines[1:])
    return f'"""{lines[0]}\n{body}\n{indent}"""'


def apply_replacements(
    original: bytes, replacements: list[tuple[int, int, bytes]]
) -> bytes:
    # apply from the end so earlier offsets stay valid
    result = original
    for start, end, text in sorted(replacements, key=lambda r: r[0], reverse=True):
        result = result[:start] + text + result[end:]
    return result


if __name__ == "__main__":
    main()
