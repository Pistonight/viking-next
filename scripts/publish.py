from typing import Any
import tomli
import subprocess

# pyright: reportAny=false

def load(path: str) -> dict[str, Any]:
    with open(path, "rb") as f:
        return tomli.load(f)
cargo_version = load("Cargo.toml")["workspace"]["package"]["version"]

subprocess.check_call(["git", "tag", f"v{cargo_version}"])
subprocess.check_call(["git", "push", "origin", f"v{cargo_version}"])
