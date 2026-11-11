"""Implementation for the File List YAML Format"""

from ._core import _core_fl
from .core import core_state

AddressLabel = _core_fl.AddressLabel
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

    Raises exception if the input `hex_value` does not convert to a valid
    `u32` offset.
    """
    return _core_fl.parse_abs_address_as_offset(core_state, hex_value)


def parse_abs_address(hex_value: str) -> int:
    """Parse an absolute address (offset + base address) as a `u64`

    Raises exception if the input `hex_value` does not convert to a valid
    `u32` offset, according to the base address in the config"""
    return _core_fl.parse_abs_address(core_state, hex_value)
