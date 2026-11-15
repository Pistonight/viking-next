from . import State
from collections.abc import Sequence
from typing import Any, Final, final

__all__ = ["AddressLabel", "Status", "is_csv", "parse_abs_address", "parse_abs_address_as_offset", "parse_offset"]

class AddressLabel:
    """Label for an address

    ```rust
    pub enum AddressLabel {
        /// Address has a single name
        Single(String),
        /// Adderss has multiple names.
        ///
        /// For example, constructor C1/C2 or destructor D1/D2
        /// are usually the same function.
        Multi(Vec<String>),
    }
    ```
    """
    def __str__(self, /) -> str: ...
    def all(self, /) -> list[str]:
        """Get all names"""
    def contains_name(self, /, name: str) -> bool:
        """Check if any name matches the input name"""
    def len(self, /) -> int:
        """Get the number of names"""
    def name(self, /) -> str:
        """Get the name (first if multiple)"""
    @final
    class Multi(AddressLabel):
        """@private"""
        __match_args__: Final = ("_0",)
        @property
        def _0(self, /) -> list[str]: ...
        def __getitem__(self, /, key: int) -> Any: ...
        def __len__(self, /) -> int: ...
        def __new__(cls, /, _0: Sequence[str]) -> AddressLabel.Multi: ...
    @final
    class Single(AddressLabel):
        """@private"""
        __match_args__: Final = ("_0",)
        @property
        def _0(self, /) -> str: ...
        def __getitem__(self, /, key: int) -> Any: ...
        def __len__(self, /) -> int: ...
        def __new__(cls, /, _0: str) -> AddressLabel.Single: ...

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
        NotDecompiled,
        /// Working in progress
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
