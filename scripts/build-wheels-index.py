"""
Generate a Python package index (PEP 503 "simple" API) at dist/docs/wheels/
for the wheels uploaded to GitHub releases.

    dist/docs/wheels/index.html                 -> lists the project
    dist/docs/wheels/<project>/index.html       -> links to every wheel

The pages are named index.html so GitHub Pages serves them as text/html
for the directory URLs (pip and uv reject other content types).
The wheel links point to the release assets on GitHub, with the sha256
digest from the API.

The GitHub token is read from GITHUB_TOKEN or GH_TOKEN. It's optional
locally since the repo is public, but required in CI (CI is set) to avoid
the rate limit for unauthenticated requests.

Only uses the standard library, so it can run without the project environment.
"""

import html
import json
import os
import re
import sys
import tomllib
import urllib.request
from pathlib import Path
from typing import Any

# pyright: reportAny=false, reportExplicitAny=false

ROOT = Path(__file__).resolve().parent.parent
OUTPUT_DIR = ROOT / "dist" / "docs" / "wheels"
DEFAULT_REPO = "Pistonight/viking-next"
API_URL = "https://api.github.com"

LINK_NEXT_RE = re.compile(r'<([^>]+)>;\s*rel="next"')


def normalize(name: str) -> str:
    """PEP 503 name normalization"""
    return re.sub(r"[-_.]+", "-", name).lower()


def get_token() -> str | None:
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if not token and os.environ.get("CI"):
        print(
            "error: GITHUB_TOKEN or GH_TOKEN must be set in CI",
            file=sys.stderr,
        )
        sys.exit(1)
    return token


def fetch_releases(repo: str, token: str | None) -> list[dict[str, Any]]:
    """Fetch all releases (with assets), following the pagination links"""
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "build-wheels-index",
    }
    if token:
        headers["Authorization"] = f"Bearer {token}"

    releases: list[dict[str, Any]] = []
    url: str | None = f"{API_URL}/repos/{repo}/releases?per_page=100"
    while url:
        request = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(request) as response:
            releases.extend(json.load(response))
            link = response.headers.get("Link") or ""
        match = LINK_NEXT_RE.search(link)
        url = match.group(1) if match else None
    return releases


def get_wheel_links(
    releases: list[dict[str, Any]], project: str, requires_python: str | None
) -> list[str]:
    links: list[str] = []
    for release in releases:
        if release["draft"]:
            continue
        for asset in release["assets"]:
            name: str = asset["name"]
            # wheel names start with the distribution name, with - replaced by _
            if not name.endswith(".whl") or normalize(name.split("-")[0]) != project:
                continue
            href = asset["browser_download_url"]
            digest: str | None = asset.get("digest")
            if digest and digest.startswith("sha256:"):
                href += f"#sha256={digest.removeprefix('sha256:')}"
            else:
                print(
                    f"warning: {release['tag_name']}/{name} has no sha256 digest",
                    file=sys.stderr,
                )
            attrs = f'href="{html.escape(href)}"'
            if requires_python:
                attrs += f' data-requires-python="{html.escape(requires_python)}"'
            links.append(f"    <a {attrs}>{html.escape(name)}</a><br>")
            print(f"found {release['tag_name']}/{name}")
    return links


def write_page(path: Path, title: str, body: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "<!DOCTYPE html>",
        "<html>",
        "  <head>",
        '    <meta name="pypi:repository-version" content="1.0">',
        f"    <title>{html.escape(title)}</title>",
        "  </head>",
        "  <body>",
        f"    <h1>{html.escape(title)}</h1>",
        *body,
        "  </body>",
        "</html>",
        "",
    ]
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    with open(ROOT / "pyproject.toml", "rb") as f:
        project_table = tomllib.load(f)["project"]
    project = normalize(project_table["name"])
    # Applied to all wheels: releases older than a requires-python change
    # would be listed with the new value
    requires_python: str | None = project_table.get("requires-python")

    repo = os.environ.get("GITHUB_REPOSITORY") or DEFAULT_REPO
    releases = fetch_releases(repo, get_token())
    links = get_wheel_links(releases, project, requires_python)
    if not links:
        print(f"warning: no wheels found in the releases of {repo}", file=sys.stderr)

    write_page(
        OUTPUT_DIR / "index.html",
        "Simple index",
        [f'    <a href="{project}/">{project}</a><br>'],
    )
    write_page(OUTPUT_DIR / project / "index.html", f"Links for {project}", links)
    print(f"wrote {len(links)} wheel(s) to {OUTPUT_DIR.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
