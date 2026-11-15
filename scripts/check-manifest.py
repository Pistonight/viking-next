"""
Consistency checks for the project manifests (Cargo.toml and pyproject.toml).

- Cargo.toml and pyproject.toml must declare the same version
- src/viking_core/Cargo.toml must have empty default features

Uses tomli instead of tomllib because tomllib only supports TOML 1.0,
and Cargo accepts TOML 1.1.
"""

import sys
from typing import Any

import tomli

# pyright: reportAny=false


def load(path: str) -> dict[str, Any]:
    with open(path, "rb") as f:
        return tomli.load(f)


def check_version() -> bool:
    cargo_version = load("Cargo.toml")["workspace"]["package"]["version"]
    py_version = load("pyproject.toml")["project"]["version"]

    if cargo_version != py_version:
        print(
            f"error: version mismatch: Cargo.toml has {cargo_version}, pyproject.toml has {py_version}",
            file=sys.stderr,
        )
        print("  update both to the same version", file=sys.stderr)
        return False

    print(f"version check: ok ({cargo_version})")
    return True


def check_default_features() -> bool:
    path = "src/viking_core/Cargo.toml"
    default = load(path).get("features", {}).get("default")

    if default != []:
        print(f"error: default features must be empty in {path}", file=sys.stderr)
        print(f"  expected: default = [], found: default = {default}", file=sys.stderr)
        print(
            "  if you enabled features for local development (e.g. rust-analyzer),",
            file=sys.stderr,
        )
        print(
            "  revert it, or set the feature set in your local rust-analyzer config instead",
            file=sys.stderr,
        )
        return False

    print("default feature check: ok")
    return True


def main() -> None:
    # run all checks so every problem is reported at once
    results = [check_version(), check_default_features()]
    if not all(results):
        sys.exit(1)


if __name__ == "__main__":
    main()
