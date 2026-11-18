import json
import os
import platform
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.parse
import urllib.request
from importlib import metadata
from pathlib import Path
from typing import Any

from .. import ui

_DIST_NAME = "nx-decomp-tools-viking"
_REPO_URL = "https://github.com/Pistonight/viking-next"
_CRATE_NAME = "viking"
_BIN_NAME = "viking"
_CARGO_FEATURES = "global-allocator,cmd-all"


def rust_cli_install_source(
    cargo_root: str | Path,
    cargo_flags: list[str] | None = None,
    repo: str | None = None,
    commit: str | None = None,
) -> Path:
    """Build and install the Rust CLI from source with `cargo install`

    The same version as the installed Python package is installed.
    The binary is installed to `<cargo_root>/bin/viking`, and returned.
    Cargo stores tracking info in `cargo_root`, so re-installing the
    version that is already installed is a no-op.

    `cargo_flags` are additional flags passed to `cargo install`.

    Use `repo` or `commit` to force a different source than what would be
    discovered from the current installation
    """
    cargo_root = Path(cargo_root)
    command = [
        "cargo",
        "install",
        _CRATE_NAME,
        *_cargo_source_args(repo, commit),
        "--root",
        str(cargo_root),
        "--features",
        _CARGO_FEATURES,
        *(cargo_flags or []),
    ]
    ui.step("installing Rust CLI from source...")
    try:
        subprocess.check_call(command)
    except FileNotFoundError:
        ui.bail("cargo not found, install the Rust toolchain and try again")
    except subprocess.CalledProcessError:
        ui.bail("failed to install Rust CLI from source")
    return cargo_root / "bin" / _BIN_NAME


def _cargo_source_args(repo: str | None, commit: str | None) -> list[str]:
    """`cargo install` arguments to build the same version as the Python package,
    or the passed-in override repo/commit

    - installed from git (e.g. `uv add git+https://...`): the same commit,
      from the same repo (which may be a fork)
    - installed from a local directory (e.g. development): that directory
    - otherwise (e.g. pre-built wheel): the release tag of the package version

    The info comes from direct_url.json (PEP 610), written by the installer
    when a package is installed from a URL instead of an index
    """
    if repo and commit:
        return ["--git", repo, "--rev", commit]

    direct_url_json = metadata.distribution(_DIST_NAME).read_text("direct_url.json")
    if direct_url_json:
        direct_url: dict[str, Any] = json.loads(direct_url_json)  # pyright: ignore[reportAny]
        vcs_info: dict[str, Any] | None = direct_url.get("vcs_info")
        if vcs_info and vcs_info.get("vcs") == "git":
            vcs_commit = vcs_info.get("commit_id")
            if vcs_commit is not None:
                vcs_commit: str | None = str(vcs_commit)
            vcs_url = direct_url.get("url", "")  # pyright: ignore[reportAny]
            if vcs_url:
                vcs_url: str | None = str(vcs_url)
            the_url = repo or vcs_url
            the_commit = commit or vcs_commit
            if the_url and the_commit:
                # url is the repo it was installed from, without the "git+" prefix
                return ["--git", the_url, "--rev", the_commit]
        if "dir_info" in direct_url:
            url = direct_url.get("url", "")  # pyright: ignore[reportAny]
            if url:
                url: str = str(url)
            if url.startswith("file://"):
                path = Path(
                    urllib.request.url2pathname(urllib.parse.urlparse(url).path)
                )
                return ["--path", str(path / "src" / "viking_core")]
    return ["--git", repo if repo else _REPO_URL, "--tag", _version_tag()]


def rust_cli_install_pre_built(path: str | Path) -> Path:
    """Download the pre-built Rust CLI from the GitHub release to `path`

    The release of the same version as the installed Python package is used.
    `path` is the path of the binary itself (not a directory), and is returned.
    """
    arch = platform.machine().lower()
    if arch == "arm64":
        arch = "aarch64"
    elif arch == "x64" or arch == "amd64":
        arch = "x86_64"

    system = platform.system()
    TARGET_TRIPLES = {
        ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
        ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
        ("Darwin", "aarch64"): "aarch64-apple-darwin",
    }
    target = TARGET_TRIPLES.get((system, arch))
    if target is None:
        ui.bail(
            f"no pre-built Rust CLI for {system}-{arch}, please install from source"
        )

    path = Path(path)
    tag = _version_tag()
    url = f"{_REPO_URL}/releases/download/{tag}/{_BIN_NAME}-{target}-{tag}.tar.gz"

    ui.step(f"downloading Rust CLI from {url}...")
    with tempfile.TemporaryDirectory() as tmpdir:
        archive = Path(tmpdir) / "viking.tar.gz"
        try:
            _ = urllib.request.urlretrieve(url, archive)
        except OSError as e:  # includes urllib.error.URLError/HTTPError
            ui.bail(f"failed to download Rust CLI: {e}")
        with tarfile.open(archive) as f:
            bin_name = _BIN_NAME if sys.platform != "win32" else (_BIN_NAME + ".exe")
            member = f.extractfile(bin_name)
            if member is None:
                ui.bail(f"{bin_name} not found in the downloaded archive")
            data = member.read()

    path.parent.mkdir(parents=True, exist_ok=True)
    # remove first instead of overwriting, in case the file is hardlinked
    # (see rust_cli_hardlinks), so the links are not modified
    path.unlink(missing_ok=True)
    _ = path.write_bytes(data)
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    ui.step(f"installed Rust CLI to {path}")
    return path


def rust_cli_create_hardlinks(viking_bin: str | Path, links: list[str | Path]) -> None:
    """Create hardlinks to the Rust CLI binary

    The command is inferred from the name of the link, so for example,
    `tools/check` would be the same as `tools/viking check`.
    Existing files at the link paths are replaced.

    Installing the binary again (from source or pre-built) replaces it with a
    new file, and the existing links would still point to the old binary.
    Call this again after installing to update the links.
    """
    viking_bin = Path(viking_bin)
    if not viking_bin.is_file():
        ui.bail(f"Rust CLI binary not found: {viking_bin}")
    for link in links:
        link = Path(link)
        if link.exists() and os.path.samefile(link, viking_bin):
            continue
        link.parent.mkdir(parents=True, exist_ok=True)
        link.unlink(missing_ok=True)
        os.link(viking_bin, link)
    ui.step(f"linked Rust CLI to {len(links)} path(s)")


def _version_tag() -> str:
    """Release tag of the installed Python package"""
    return "v" + metadata.version(_DIST_NAME)
