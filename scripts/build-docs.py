"""
Generate API docs with pdoc, working around a few incompatibilities
between pdoc and the PyO3 extension module (viking._core).

Usage: uv run --no-sync scripts/build-docs.py
(run through `task doc`, which also regenerates the stubs and sets
VIKING_CONFIG so that importing viking.core works without a project)

The output is written to dist/docs
"""

# LLM Generated

# This script patches pdoc internals on purpose
# pyright: reportPrivateUsage=false

import ast
import importlib.machinery
import importlib.util
import sys
import types
from collections.abc import Callable
from pathlib import Path
from typing import cast, override

import pdoc
import pdoc.doc
import pdoc.doc_pyi

ROOT = Path(__file__).resolve().parent.parent
OUTPUT_DIR = ROOT / "dist" / "docs"

# Submodules of viking left out of the docs
# - asm_differ: has import side effects and is internal
EXCLUDED_MODULES = ["asm_differ"]

# The hacks below replace private functions in pdoc, so pdoc is pinned
# to a major version in pyproject.toml. Fail loudly instead of silently
# producing broken or untyped docs if they change in an update.
for _name in (
    "_import_stub_file",
    "_prepare_module",
    "_patch_doc",
    "include_typeinfo_from_stub_files",
):
    if not hasattr(pdoc.doc_pyi, _name):
        raise RuntimeError(
            f"pdoc.doc_pyi.{_name} no longer exists; scripts/build-docs.py needs to be updated for this version of pdoc"
        )
if not hasattr(pdoc.doc, "_safe_getdoc"):
    raise RuntimeError(
        "pdoc.doc._safe_getdoc no longer exists; scripts/build-docs.py needs to be updated for this version of pdoc"
    )


# HACK 1: Make the stubs executable
#
# pdoc reads type information from .pyi files by *executing* them as regular
# Python modules (see pdoc.doc_pyi._import_stub_file). maturin generates
# PyO3 complex enums as nested classes that inherit from their enclosing class:
#
#     class ConfigDecompMeTarget:
#         class CompilerAndFlags(ConfigDecompMeTarget): ...
#
# This is fine for type checkers (which only parse the stub), but at runtime
# the outer class name is not bound until its body finishes executing, so the
# base expression raises NameError. pdoc then discards the *entire* stub, and
# we lose all type information for the module.
#
# We work around this by rewriting the stub's AST before executing it: any
# base class of a nested class that refers to an enclosing class is replaced
# with `object`. This loses nothing in the output, because pdoc only takes
# signatures, annotations and docstrings from the stub. The class hierarchy
# displayed in the docs comes from the real classes in the extension module,
# which have the correct bases.
#
# The .pyi files on disk are not modified.


class _NestedBaseRewriter(ast.NodeTransformer):
    """Replace base classes that refer to an enclosing class with `object`"""

    def __init__(self) -> None:
        self.enclosing: list[str] = []

    @override
    def visit_ClassDef(self, node: ast.ClassDef) -> ast.ClassDef:
        node.bases = [
            ast.Name(id="object", ctx=ast.Load())
            if isinstance(base, ast.Name) and base.id in self.enclosing
            else base
            for base in node.bases
        ]
        self.enclosing.append(node.name)
        _ = self.generic_visit(node)
        _ = self.enclosing.pop()
        return node


class _StubLoader(importlib.machinery.SourceFileLoader):
    """Loads a .pyi stub with _NestedBaseRewriter applied"""

    @override
    def get_code(self, fullname: str) -> types.CodeType:
        # Always compile from source. The default implementation may load
        # cached bytecode in __pycache__ compiled from the unpatched source
        path = self.get_filename(fullname)
        tree = ast.parse(self.get_data(path), filename=path)
        tree = cast(ast.Module, _NestedBaseRewriter().visit(tree))
        tree = ast.fix_missing_locations(tree)
        return compile(tree, path, "exec", dont_inherit=True)


def _import_stub_file(module_name: str, stub_file: Path) -> types.ModuleType:
    """Replacement for pdoc.doc_pyi._import_stub_file using _StubLoader"""
    loader = _StubLoader(module_name, str(stub_file))
    spec = importlib.util.spec_from_file_location(module_name, stub_file, loader=loader)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


pdoc.doc_pyi._import_stub_file = _import_stub_file


# HACK 2: Break self-referencing class cycles when applying stubs
#
# Once a stub loads successfully (thanks to HACK 1), pdoc applies it by
# recursively walking the members of every class (_prepare_module and
# _patch_doc). With PyO3 complex enums, each variant class is a subclass of
# the enum, so it *inherits* the variant attributes from it:
#
#     ConfigDecompMeTarget.CompilerAndFlags.CompilerAndFlags is ConfigDecompMeTarget.CompilerAndFlags
#
# pdoc follows this self-reference forever and crashes with RecursionError.
# We wrap both functions to skip a class that was already visited.
#
# The same class is also reachable through inherited aliases that are not
# rendered, e.g. ConfigDecompMeTarget.Preset.CompilerAndFlags, and these may
# be visited before the real path. _prepare_module only precomputes members,
# so skipping either path there is fine. _patch_doc must patch the real path,
# so it only marks a class as visited when the stub has an entry at that path.
# Inherited aliases never do, and pdoc does not recurse into them.

# pdoc's own annotations are generic without type arguments, so give the
# originals concrete types
_prepare_module_original = cast(
    Callable[[pdoc.doc.Namespace[types.ModuleType | type]], None],
    pdoc.doc_pyi._prepare_module,
)
_patch_doc_original = cast(
    Callable[[pdoc.doc.Doc[object], pdoc.doc.Module], None],
    pdoc.doc_pyi._patch_doc,
)
_include_typeinfo_original = pdoc.doc_pyi.include_typeinfo_from_stub_files
_prepare_module_visited: set[int] = set()
_patch_doc_visited: set[int] = set()


def _prepare_module(ns: pdoc.doc.Namespace[types.ModuleType | type]) -> None:
    if id(ns.obj) in _prepare_module_visited:
        return
    _prepare_module_visited.add(id(ns.obj))
    _prepare_module_original(ns)


def _patch_doc(target_doc: pdoc.doc.Doc[object], stub_mod: pdoc.doc.Module) -> None:
    # Python 3.14 (PEP 649) gives classes with annotations an __annotate_func__
    # function, which never matches the extension class's attribute and only
    # produces "Stub is a function, but target is a ..." warnings. It's not
    # rendered, so there's nothing to patch
    if target_doc.name == "__annotate_func__":
        return
    if isinstance(target_doc, pdoc.doc.Class):
        if id(target_doc.obj) in _patch_doc_visited:
            return
        if stub_mod.get(target_doc.qualname) is not None:  # pyright: ignore[reportUnknownMemberType]
            _patch_doc_visited.add(id(target_doc.obj))
    _patch_doc_original(target_doc, stub_mod)


def _include_typeinfo_from_stub_files(module: pdoc.doc.Module) -> None:
    # stubs are applied once per module; reset the visited state for each
    _prepare_module_visited.clear()
    _patch_doc_visited.clear()
    _include_typeinfo_original(module)
    _apply_core_stubs_to_reexports(module)  # HACK 6


pdoc.doc_pyi._prepare_module = _prepare_module
pdoc.doc_pyi._patch_doc = _patch_doc
pdoc.doc_pyi.include_typeinfo_from_stub_files = _include_typeinfo_from_stub_files


# HACK 3: Make the PyO3 submodules discoverable
#
# PyO3 submodules (viking._core._core_fl, etc.) are plain module objects
# attached as attributes of the extension module. At runtime they are named
# like "_core._core_fl", which pdoc does not recognize as children of
# "viking._core", so they are left out of the docs entirely. They are also not
# importable as "viking._core._core_fl" since the extension module is not a
# package.
#
# We fix this by:
# - renaming them to their full dotted name and registering them in
#   sys.modules, so pdoc can import them by name, list them as submodules
#   of viking._core, and find the matching .pyi stub
#   (e.g. src/viking/_core/_core_fl.pyi)
# - giving them a __spec__, because pdoc looks modules up with
#   importlib.util.find_spec, which rejects modules whose __spec__ is None
# - passing them to pdoc explicitly, because pdoc only walks into packages
#   (modules with __path__) to find submodules, and the extension module
#   is not one
#
# This only affects the docs build process.


def _register_core_submodules() -> list[str]:
    """Register the PyO3 submodules and return their full names"""
    from viking import _core

    names: list[str] = []
    _index_core_classes(_core)
    for name in _core.__all__:
        member = cast(object, getattr(_core, name))
        if not isinstance(member, types.ModuleType):
            continue
        full_name = f"{_core.__name__}.{name}"
        member.__name__ = full_name
        member.__spec__ = importlib.machinery.ModuleSpec(full_name, None)
        sys.modules[full_name] = member
        names.append(full_name)
        _index_core_classes(member)
    return names


# HACK 6: Apply the _core stubs to classes re-exported from public modules
#
# Public modules re-export _core classes for consumers, for example in
# viking/core.py:
#
#     Config = _core.Config
#
# pdoc documents the class again on the re-exporting module's page, but it
# only applies a stub to the module the stub belongs to. viking/core.py has no
# stub, so the re-exported class would lose all type information (properties
# show up as untyped variables).
#
# After pdoc applies stubs to a module, we look for classes in it that are
# actually _core classes (by object identity), and apply the stub of the _core
# module that defines them as well.
#
# Limitation: the stub is looked up by the class name in the re-exporting
# module, so a class re-exported under a different name
# (e.g. `Settings = _core.Config`) is not typed.

# id of each top-level _core class -> name of the _core module defining it
_core_class_modules: dict[int, str] = {}
# _core module name -> (executed stub module, pdoc doc of the stub)
_core_stubs: dict[str, tuple[types.ModuleType, pdoc.doc.Module] | None] = {}


def _index_core_classes(module: types.ModuleType) -> None:
    for value in cast(dict[str, object], vars(module)).values():
        if isinstance(value, type):
            _core_class_modules[id(value)] = module.__name__


def _load_core_stub(
    module_name: str,
) -> tuple[types.ModuleType, pdoc.doc.Module] | None:
    if module_name not in _core_stubs:
        stub_file = pdoc.doc_pyi.find_stub_file(module_name)
        if stub_file is None:
            _core_stubs[module_name] = None
        else:
            imported_stub = _import_stub_file(module_name, stub_file)
            _core_stubs[module_name] = (imported_stub, pdoc.doc.Module(imported_stub))
    return _core_stubs[module_name]


def _apply_core_stubs_to_reexports(module: pdoc.doc.Module) -> None:
    from unittest import mock

    if module.modulename in _core_class_modules.values():
        return  # a _core module itself, its own stub was already applied
    # pdoc's generics are invariant, so cast to the types our wrappers take
    members = cast(dict[str, pdoc.doc.Doc[object]], module.members)
    for member in members.values():
        if not isinstance(member, pdoc.doc.Class):
            continue
        core_module = _core_class_modules.get(id(member.obj))
        if core_module is None:
            continue
        stub = _load_core_stub(core_module)
        if stub is None:
            continue
        imported_stub, stub_doc = stub
        # same steps as pdoc.doc_pyi.include_typeinfo_from_stub_files
        _prepare_module(cast(pdoc.doc.Namespace[types.ModuleType | type], member))
        with mock.patch.dict("sys.modules", {core_module: imported_stub}):
            _patch_doc(cast(pdoc.doc.Doc[object], member), stub_doc)


# HACK 5: Don't use NoneType's docstring for undocumented functions
#
# When a function has no docstring (e.g. a Rust function without `///` docs),
# pdoc falls back to looking it up on its owner: it finds the owner from
# __module__ in sys.modules and __qualname__. PyO3 objects don't support this
# lookup (functions have __module__ like "_core._core_ui" which is not in
# sys.modules, and methods have __module__ = None), so the lookup ends with
# None and pdoc calls inspect.getdoc(None). That returns the docstring of
# NoneType: "The type of the None singleton."
#
# We patch pdoc's getdoc helper to treat None as having no docstring.

_safe_getdoc_original = pdoc.doc._safe_getdoc


def _safe_getdoc(obj: object) -> str:
    if obj is None:
        return ""
    return _safe_getdoc_original(obj)


pdoc.doc._safe_getdoc = _safe_getdoc


def main() -> None:
    core_submodules = _register_core_submodules()
    pdoc.pdoc(
        "viking",
        *core_submodules,
        *(f"!viking.{name}" for name in EXCLUDED_MODULES),
        output_directory=OUTPUT_DIR,
    )


if __name__ == "__main__":
    main()
