from . import State
from collections.abc import Sequence
from typing import Final, final

__all__ = ["Info", "Status", "is_csv", "parse_abs_address", "parse_abs_address_as_offset", "parse_offset"]

@final
class Info:
    """Clone-on-write handle to an [`InfoData`]"""
    def __new__(cls, /, state: State, names: Sequence[str]) -> Info: ...
    def __str__(self, /) -> str: ...
    @property
    def end(self, /) -> int:
        """Get the (absolute) end address in the original ELF"""
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
        """Get the name (first if multiple)"""
    @property
    def names(self, /) -> list[str]:
        """Get all names"""
    @property
    def names_len(self, /) -> int:
        """Get the number of names"""
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
        """Get the (absolute) start address in the original ELF"""
    @property
    def status(self, /) -> Status:
        """Decompilation status of the symbol"""
    @status.setter
    def status(self, /, status: Status) -> None:
        """Decompilation status of the symbol"""

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
def parse_abs_address(state: State, hex_value: str) -> int: ...
def parse_abs_address_as_offset(state: State, hex_value: str) -> int: ...
def parse_offset(hex_value: str) -> int: ...
