"""
Generate the Rust API docs with rustdoc (nightly), at target/doc-nightly/doc/

After cargo doc:
- index.html at the doc root is replaced with a redirect to the viking crate
- every page with the crate name link at the top left of the sidebar gets a
  "(home)" link next to it, to the project home page

The docs are deployed to dist/docs/rust/ (see `task doc` and the doc
workflow), and the home page (the README rendered by build-markdown-doc.py)
is dist/docs/index.html, one level above the doc root.

Only uses the standard library, so CI can run it without setting up uv.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET_DIR = ROOT / "target" / "doc-nightly"
DOC_DIR = TARGET_DIR / "doc"
INDEX_HTML = DOC_DIR / "index.html"
# the crate the doc root redirects to
ROOT_CRATE = "viking"
# home page, relative to the doc root
HOME_PAGE = "../index.html"

CARGO_DOC_ARGS = [
    "doc",
    "--target-dir",
    str(TARGET_DIR),
    "--no-deps",
    "--no-default-features",
    "--features",
    "cmd-all,nightly",
    "-p",
    "viking",
    "-p",
    "viking-macro",
]

# Marks pages that already have the home link, since cargo doc may not
# rewrite every page on each run
HOME_LINK_CLASS = "viking-home-link"

# The crate name link in the sidebar:
#   <div class="sidebar-crate"><h2><a href="../viking/index.html">viking</a>
# The name may contain <wbr> tags (e.g. viking_<wbr>macro).
# Pages not belonging to a crate (settings.html, help.html) link to
# "./index.html" with the text "Rustdoc", which doesn't match.
CRATE_LINK_RE = re.compile(
    r'(<div class="sidebar-crate">(?:<a class="logo-container"[^>]*>.*?</a>)?<h2>)'
    + r'(<a href="(?:[^"]*/)?\w+/index\.html">(?:[^<]|<wbr>)*</a>)'
)

# rustdoc displays the crate link as a block, with negative margins and
# padding to make the hover highlight span the sidebar (`.sidebar-crate h2 a`
# in rustdoc's CSS). Move that box to the row containing both links, so
# hovering anywhere on the row highlights the full width like before. The
# links inside are plain inline text with no background of their own, and
# the home link uses the sidebar link color.
STYLE = f"""<style>
.sidebar .sidebar-crate h2 .viking-home-row {{
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    column-gap: 0.4em;
    background-clip: border-box;
    margin: 0 calc(-24px + 0.25rem) 0 calc(-0.2rem - var(--sidebar-elems-left-padding));
    padding: calc((16px - 0.57rem) / 2) 0.25rem;
    padding-left: calc(0.2rem + var(--sidebar-elems-left-padding));
}}
.sidebar .sidebar-crate h2 .viking-home-row:hover {{
    background-color: var(--sidebar-current-link-background-color);
}}
.sidebar .sidebar-crate h2 .viking-home-row > a {{
    display: inline;
    margin: 0;
    padding: 0;
    background-color: transparent;
}}
.sidebar .sidebar-crate h2 .viking-home-row > a.{HOME_LINK_CLASS} {{
    color: var(--sidebar-link-color);
    font-size: 1rem;
    font-weight: normal;
}}
/* the whole row highlights on hover, so underline the home link to show
 * which link is being clicked */
.sidebar .sidebar-crate h2 .viking-home-row > a.{HOME_LINK_CLASS}:hover {{
    text-decoration: underline;
}}
</style>"""


def run_cargo_doc() -> None:
    cmd = ["cargo", *CARGO_DOC_ARGS]
    if not os.environ.get("CI"):
        # CI already installs the correct toolchain. For local development,
        # explicitly request any nightly toolchain
        cmd.insert(1, "+nightly")
    print(" ".join(cmd), flush=True)
    result = subprocess.run(cmd, check=False, cwd=ROOT)
    if result.returncode != 0:
        sys.exit(result.returncode)


def write_index_redirect() -> None:
    print("Generating index redirect")
    _ = INDEX_HTML.write_text(
        "<!DOCTYPE html>\n"
        + "<html><head>\n"
        + f'<meta http-equiv="refresh" content="0;url={ROOT_CRATE}/index.html">\n'
        + "</head><body></body></html>\n",
        encoding="utf-8",
    )


def add_home_link(path: Path) -> bool:
    """Add the home link to a page, return whether it was changed"""
    content = path.read_text(encoding="utf-8")
    if HOME_LINK_CLASS in content:
        return False
    # relative from the page's directory, e.g. doc/viking/fl/x.html -> ../../../index.html
    home = os.path.relpath(DOC_DIR / HOME_PAGE, path.parent).replace(os.sep, "/")

    def replace(m: re.Match[str]) -> str:
        return (
            f'{m.group(1)}<span class="viking-home-row">{m.group(2)}'
            + f'<a class="{HOME_LINK_CLASS}" href="{home}">(home)</a></span>'
        )

    content, count = CRATE_LINK_RE.subn(replace, content, count=1)
    if count == 0:
        return False
    content = content.replace("</head>", f"{STYLE}</head>", 1)
    _ = path.write_text(content, encoding="utf-8")
    return True


def add_home_links() -> None:
    changed = sum(add_home_link(path) for path in DOC_DIR.rglob("*.html"))
    print(f"Added home link to {changed} pages")


def main() -> None:
    run_cargo_doc()
    write_index_redirect()
    add_home_links()


if __name__ == "__main__":
    main()
