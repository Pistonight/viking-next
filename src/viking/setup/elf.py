import subprocess
from pathlib import Path

from .. import ui
from ..core import find_tool, get_config


def convert_nso_to_elf(
    nso_path: Path,
    elf_out_path: str | Path = "",
    uncompressed_nso_out_path: str | Path | None = None,
    message: str = "converting NSO to ELF...",
):
    """Convert a compressed or uncompressed NSO to ELF, optionally exporting the uncompressed NSO

    If elf_out_path is not given, defaults to the versionless original elf path
    as defined in the config TOML (see `viking.core.ConfigRepo`)
    """

    ui.step(message)
    if not elf_out_path:
        config = get_config()
        elf_out_path = str(config.repo.original_elf())
    command = [find_tool("nx2elf"), str(nso_path), "--export-elf", elf_out_path]
    if uncompressed_nso_out_path is not None:
        command.append("--export-uncompressed")
        command.append(uncompressed_nso_out_path)
    subprocess.check_call(command)


def apply_xdelta3_patch(
    input: Path, patch: Path, dest: Path, message: str = "applying patch..."
):
    """Apply `patch` to `input` and save the result to `dest`"""
    ui.step(message)
    try:
        subprocess.check_call(
            ["xdelta3", "-d", "-s", str(input), str(patch), str(dest)]
        )
    except FileNotFoundError:
        ui.bail("install xdelta3 and try again")
