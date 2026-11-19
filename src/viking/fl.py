"""File list utils"""

from collections.abc import Sequence
from pathlib import Path

from ._core import _core_fl
from .core import core_state


def load_file_list(version: str | None = None) -> FileList:
    """Load the optionally-versioned file list according to the paths in the config."""
    return _core_fl.load_file_list(core_state, version)


def load_file_list_at(path: str | Path) -> FileList:
    """Load the file list from a path, auto-detecting the format

    If the path ends with .csv, it's assumed to be the legacy CSV format.
    Otherwise it's assumed to be the new YAML format."""
    return _core_fl.load_file_list_at(core_state, str(path))


def load_file_list_csv_at(path: str | Path) -> FileList:
    """Load the file list from a path, assuming legacy CSV format.

    All symbols are put into an "UNKNOWN" object."""
    return _core_fl.load_file_list_csv_at(core_state, str(path))


def load_file_list_yml_at(path: str | Path) -> FileList:
    """Load the file list from a path, assuming YAML format"""
    return _core_fl.load_file_list_yml_at(core_state, str(path))


def write_file_list_csv(file_list: FileList, version: str | None = None) -> None:
    """Write the optionally-versioned file list according to the paths in
    the config (assuming CSV format).

    See `write_file_list_csv_at` for caveats."""
    _core_fl.write_file_list_csv(core_state, file_list, version)


def write_file_list_csv_at(file_list: FileList, path: str | Path) -> None:
    """Write the file list data to `path`, assuming CSV format.

    The CSV does not store object information.

    The file list is reloaded and validated before writing."""
    _core_fl.write_file_list_csv_at(file_list, str(path))


def write_file_list_yml(file_list: FileList, version: str | None = None) -> None:
    """Write the optionally-versioned file list according to the paths in
    the config (assuming YAML format)."""
    _core_fl.write_file_list_yml(core_state, file_list, version)


def write_file_list_yml_at(file_list: FileList, path: str | Path) -> None:
    """Write the file list data to `path`, assuming YAML format.

    The file list is reloaded and validated before writing."""
    _core_fl.write_file_list_yml_at(file_list, str(path))


def new_file_list(objects: Sequence[tuple[str, Object]]) -> FileList:
    """Create a new instance of `FileList` using a list of in-memory objects. Use this factory function instead of constructing one
    directly, because the constructor depends on the Rust module state.

    When used in conjunction with `FileList.get_all_objects`, you can modify the objects in Python
    and construct a new FileList to write them back to disk.
    """
    return FileList(core_state, objects)


FileList = _core_fl.FileList
Object = _core_fl.Object


def new_info(names: Sequence[str]) -> Info:
    """Create a new instance of `Info`. Use this factory function instead of constructing one
    directly, because the constructor depends on the Rust module state.
    """
    return Info(core_state, names)


Info = _core_fl.Info
Status = _core_fl.Status


def is_csv(version: str | None = None) -> bool:
    """Check if the file list path is CSV"""
    return _core_fl.is_csv(core_state, version)


def parse_offset(hex_value: str) -> int:
    """Parse a hex value as-is as a `u32`"""
    return _core_fl.parse_offset(hex_value)


def parse_abs_address_as_offset(hex_value: str) -> int:
    """Parse an absolute address (offset + base address) to just the offset
    according to the base address in the config

    Raises an exception if the input `hex_value` does not convert to a valid
    `u32` offset.
    """
    return _core_fl.parse_abs_address_as_offset(core_state, hex_value)


def parse_abs_address(hex_value: str) -> int:
    """Parse an absolute address (offset + base address) as a `u64`

    Raises an exception if the input `hex_value` does not convert to a valid
    `u32` offset, according to the base address in the config"""
    return _core_fl.parse_abs_address(core_state, hex_value)
