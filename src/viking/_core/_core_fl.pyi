from . import State
from collections.abc import Sequence
from typing import Final, final

__all__ = ["FileList", "Info", "Object", "Status", "is_csv", "load_file_list", "load_file_list_at", "load_file_list_csv_at", "load_file_list_yml_at", "parse_abs_address", "parse_abs_address_as_offset", "parse_offset", "write_file_list_csv", "write_file_list_csv_at", "write_file_list_yml", "write_file_list_yml_at"]

@final
class FileList:
    """The YAML File List structure"""
    def __new__(cls, /, state: State, data: Sequence[tuple[str, Object]]) -> FileList: ...
    def get_all_objects(self, /) -> list[tuple[str, Object]]:
        """Get a list of all objects and their name, sorted by offset.

        The list is a copy. Modifying the list will not change this FileList.
        """
    def info_list(self, /) -> list[Info]:
        """Get a flat list of all `Info`.

        The underlying `Vec` that holds shared `Info` pointers is cloned to Python.
        The underlying `Info`s will only be cloned on write. Modifying the returned list will not
        modify the file list.

        To modify the FileList, use the mutater methods (`set_*` and `update_*`)
        """
    def ith_object(self, /, index: int) -> tuple[str, Object]:
        """Get the name and data of the `i`-th object. Raises exception if index is out of bound.

        The returned object is a copy. To modify the FileList, use the mutater methods (`set_*` and `update_*`)
        """
    def object_from_name(self, /, name: str) -> Object | None:
        """Search for the object with the name and return its data.

        The returned object is a copy. To modify the FileList, use the mutater methods (`set_*` and `update_*`)
        """
    def object_from_offset(self, /, offset: int) -> tuple[str, Object] | None:
        """Search for the object containing the offset and return its name and data.

        The returned object is a copy. To modify the FileList, use the mutater methods (`set_*` and `update_*`)
        """
    def object_index_from_offset(self, /, offset: int) -> int | None:
        """Search for the object containing the offset and return its index.

        `None` is returned if the offset is less than the first object.
        If the offset is larger than the last object, the last object is returned.
        """
    @property
    def objects_len(self, /) -> int:
        """Number of objects"""
    def rename_object(self, /, name: str, new_name: str) -> None:
        """Set the new name of the object with the given name"""
    def set_ith_object(self, /, index: int, object: Object) -> None:
        """Set the data of the `i`-th object."""
    def set_ith_object_name(self, /, index: int, name: str) -> None:
        """Set the name of the `i`-th object."""
    def set_object_by_name(self, /, name: str, object: Object) -> None:
        """Set the data of the object with the given name"""
    def update_at_offset(self, /, offset: int, data: Info) -> None:
        """Update a symbol data at the offset. Raises an exception if no symbol is found
        with the offset.
        """

@final
class Info:
    """Clone-on-write handle to the underlying function info (on the Rust side)"""
    def __new__(cls, /, state: State, names: Sequence[str]) -> Info: ...
    def __str__(self, /) -> str: ...
    @property
    def end(self, /) -> int:
        """The (absolute) end address in the original ELF"""
    @property
    def guess(self, /) -> bool:
        """If the symbol name is a guess instead of from some string in the original binary"""
    @guess.setter
    def guess(self, /, guess: bool) -> None:
        """If the symbol name is a guess instead of from some string in the original binary"""
    @property
    def is_decompiled(self, /) -> bool:
        """Return `true` if the function is not a library function and
        has been worked on (i.e. is not `U`)
        """
    @property
    def lazy(self, /) -> bool:
        """If the symbol's placement into a TU is lazy. Typically marked with V or W
        in the output of llvm-nm.
        """
    @lazy.setter
    def lazy(self, /, lazy: bool) -> None:
        """If the symbol's placement into a TU is lazy. Typically marked with V or W
        in the output of llvm-nm.
        """
    @property
    def name(self, /) -> str:
        """The name (first if multiple)"""
    @property
    def names(self, /) -> list[str]:
        """All names"""
    @property
    def names_len(self, /) -> int:
        """The number of names"""
    @property
    def offset(self, /) -> int:
        """The address as offset from the base address defined in the config"""
    @offset.setter
    def offset(self, /, offset: int) -> None:
        """The address as offset from the base address defined in the config"""
    def set_names(self, /, names: Sequence[str]) -> None:
        """Set the names. Raises exception if there are more than one name and any of the name is
        empty.
        """
    @property
    def size(self, /) -> int:
        """Size of the symbol at the address"""
    @size.setter
    def size(self, /, size: int) -> None:
        """Size of the symbol at the address"""
    @property
    def start(self, /) -> int:
        """The (absolute) start address in the original ELF"""
    @property
    def status(self, /) -> Status:
        """Decompilation status of the symbol"""
    @status.setter
    def status(self, /, status: Status) -> None:
        """Decompilation status of the symbol"""

@final
class Object:
    """One object entry in a YAML file list"""
    def __new__(cls, /, text_section: Sequence[Info]) -> Object: ...
    def __str__(self, /) -> str: ...
    def get_text_section(self, /) -> list[Info]:
        """Get the text section (list of function infos).

        The underlying `Vec` that holds shared `Info` pointers is cloned to Python.
        The underlying `Info`s will only be cloned on write. Modifying the returned list will not
        modify this `Object`. Use `set_text_section` for that.
        """
    @property
    def offset(self, /) -> int:
        """The start offset of the object in the original ELF"""
    def set_text_section(self, /, text_section: Sequence[Info]) -> None:
        """Set the text section (list of function infos)"""

@final
class Status:
    """Decompilation status of a function/symbol

    ```rust
    pub enum Status {
        /// Decompiled and matching
        Matching,
        /// Decompiled and not matching due to minor mismatches
        NonMatchingMinor,
        /// Decompiled and not matching due to major mismatches
        NonMatchingMajor,
        /// Unnamed or not decompiled
        #[default]
        NotDecompiled,
        /// Work in progress
        Wip,
        /// Library function - out of scope for decompilation
        Library,
    }
    ```
    """
    Library: Final[Status]
    """@private"""
    Matching: Final[Status]
    """@private"""
    NonMatchingMajor: Final[Status]
    """@private"""
    NonMatchingMinor: Final[Status]
    """@private"""
    NotDecompiled: Final[Status]
    """@private"""
    Wip: Final[Status]
    """@private"""
    def __int__(self, /) -> int: ...
    def __repr__(self, /) -> str: ...
    def __str__(self, /) -> str: ...
    def code(self, /) -> str:
        """Get the single-character code representation"""
    def description(self, /) -> str:
        """Get descriptive string"""
    @staticmethod
    def from_code(s: str) -> Status:
        """Parse status code into enum"""
    @staticmethod
    def from_name(s: str) -> Status:
        """Parse status string into enum"""

def is_csv(state: State, version: str | None) -> bool: ...
def load_file_list(state: State, version: str | None) -> FileList: ...
def load_file_list_at(state: State, path: str) -> FileList: ...
def load_file_list_csv_at(state: State, path: str) -> FileList: ...
def load_file_list_yml_at(state: State, path: str) -> FileList: ...
def parse_abs_address(state: State, hex_value: str) -> int: ...
def parse_abs_address_as_offset(state: State, hex_value: str) -> int: ...
def parse_offset(hex_value: str) -> int: ...
def write_file_list_csv(state: State, file_list: FileList, version: str | None) -> None: ...
def write_file_list_csv_at(file_list: FileList, path: str) -> None: ...
def write_file_list_yml(state: State, file_list: FileList, version: str | None) -> None: ...
def write_file_list_yml_at(file_list: FileList, path: str) -> None: ...
