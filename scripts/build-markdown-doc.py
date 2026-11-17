"""
Render the Markdown docs to HTML with the BOTW theme.

- README.md is rendered to dist/docs/index.html
- each docs/UPPER.md in ORDER is rendered to dist/docs/lower.html
- the theme lives in docs/markdown-theme (page.html, theme.css), and is output
  to dist/docs/markdown/ together with the shared files from the pdoc theme
  (palette, syntax highlighting, background) and pdoc's alert icons

Every page has a sidebar with all pages (and the table of contents of the
current page), and links to the API docs at the bottom.

Links between the Markdown files are rewritten to the rendered pages. Other
relative links (e.g. to source files) are rewritten to the file on GitHub.
"""

import html
import re
import shutil
import sys
from pathlib import Path
from string import Template

import markdown2  # pyright: ignore[reportMissingTypeStubs]
import pdoc

ROOT = Path(__file__).resolve().parent.parent
DOCS_DIR = ROOT / "docs"
THEME_DIR = DOCS_DIR / "markdown-theme"
PDOC_THEME_DIR = ROOT / "scripts" / "pdoc-theme"
PDOC_RESOURCES_DIR = Path(pdoc.__file__).parent / "templates" / "resources"
OUTPUT_DIR = ROOT / "dist" / "docs"
ASSETS_DIR = OUTPUT_DIR / "markdown"

REPO_URL = "https://github.com/Pistonight/viking-next"
# relative links to other files in the repo point here
REPO_BLOB_URL = f"{REPO_URL}/blob/main"

# docs/<name>.md, in sidebar order (README is always first)
ORDER = [
    "SETUP",
    "CONTRIBUTING",
]

# pdoc icons used by alerts ("> [!NOTE]" etc.), see theme.css
ALERT_ICONS = [
    "info-circle-fill.svg",
    "lightbulb.svg",
    "exclamation-square-fill.svg",
    "exclamation-triangle-fill.svg",
    "exclamation-octagon-fill.svg",
]

MARKDOWN_EXTRAS: dict[str, object] = {
    "alerts": None,
    "cuddled-lists": None,
    # "pdoc-code" shares the token colors with the pdoc theme
    "fenced-code-blocks": {"cssclass": "pdoc-code codehilite"},
    "footnotes": None,
    "header-ids": None,
    "strike": None,
    "tables": None,
    "task_list": None,
}


class Page:
    def __init__(self, source: Path, output_name: str, label: str) -> None:
        self.source: Path = source
        self.output_name: str = output_name
        self.label: str = label


def get_pages() -> list[Page]:
    pages = [Page(ROOT / "README.md", "index.html", "Readme")]
    for name in ORDER:
        pages.append(
            Page(DOCS_DIR / f"{name}.md", f"{name.lower()}.html", name.title())
        )
    return pages


HREF_RE = re.compile(r'href="([^"]*)"')
H1_RE = re.compile(r"<h1[^>]*>(.*?)</h1>", re.DOTALL)
H2_RE = re.compile(r'<h2 id="([^"]*)">(.*?)</h2>', re.DOTALL)
TAG_RE = re.compile(r"<[^>]+>")


def rewrite_links(
    content: str, page: Page, pages: list[Page], errors: list[str]
) -> str:
    """
    Point links to the rendered pages, or to GitHub for other repo files.
    Broken links are added to errors
    """
    rendered = {p.source.resolve(): p.output_name for p in pages}

    def rewrite(match: re.Match[str]) -> str:
        href = html.unescape(match.group(1))
        # absolute URLs, mailto:, in-page anchors
        if (
            not href
            or href.startswith("#")
            or re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", href)
        ):
            return match.group(0)
        path, sep, fragment = href.partition("#")
        target = (page.source.parent / path).resolve()
        if target in rendered:
            new_href = rendered[target]
        elif path.endswith(".md") and target.name.upper() in (
            f"{n}.MD" for n in ["README", *ORDER]
        ):
            # the docs were moved around, match by name
            new_href = next(
                p.output_name
                for p in pages
                if p.source.name.upper() == target.name.upper()
            )
        else:
            try:
                relative = target.relative_to(ROOT)
            except ValueError:
                errors.append(f"{page.source.name}: link outside of the repo: {href}")
                return match.group(0)
            if not target.exists():
                errors.append(f"{page.source.name}: broken link: {href}")
            new_href = f"{REPO_BLOB_URL}/{relative.as_posix()}"
        return f'href="{html.escape(new_href + sep + fragment)}"'

    return HREF_RE.sub(rewrite, content)


def strip_tags(text: str) -> str:
    return html.unescape(TAG_RE.sub("", text)).strip()


def render_nav(current: Page, pages: list[Page], content: str) -> str:
    lines: list[str] = []
    for page in pages:
        if page is not current:
            lines.append(
                f'      <li><a href="{page.output_name}">{html.escape(page.label)}</a></li>'
            )
            continue
        lines.append(
            f'      <li class="active"><a href="{page.output_name}">{html.escape(page.label)}</a>'
        )
        headings = list(H2_RE.finditer(content))
        if headings:
            lines.append('        <ul class="toc">')
            for heading in headings:
                lines.append(
                    f'          <li><a href="#{heading[1]}">{html.escape(strip_tags(heading[2]))}</a></li>'
                )
            lines.append("        </ul>")
        lines.append("      </li>")
    return "\n".join(lines)


def render_page(
    page: Page, pages: list[Page], template: Template, errors: list[str]
) -> None:
    markdown = page.source.read_text(encoding="utf-8")
    content = str(markdown2.markdown(markdown, extras=MARKDOWN_EXTRAS))
    content = rewrite_links(content, page, pages, errors)

    h1 = H1_RE.search(content)
    title = strip_tags(h1.group(1)) if h1 else page.label

    output = template.substitute(
        title=html.escape(title),
        repo_url=html.escape(REPO_URL),
        pages=render_nav(page, pages, content),
        content=content,
    )
    (OUTPUT_DIR / page.output_name).write_text(output, encoding="utf-8")
    print(f"rendered {page.source.relative_to(ROOT)} -> {page.output_name}")


def copy_assets() -> None:
    if ASSETS_DIR.exists():
        shutil.rmtree(ASSETS_DIR)
    (ASSETS_DIR / "icons").mkdir(parents=True)
    shutil.copy(THEME_DIR / "theme.css", ASSETS_DIR / "theme.css")
    # shared with the pdoc theme
    shutil.copy(PDOC_THEME_DIR / "theme.css", ASSETS_DIR / "palette.css")
    shutil.copy(
        PDOC_THEME_DIR / "syntax-highlighting.css",
        ASSETS_DIR / "syntax-highlighting.css",
    )
    shutil.copy(PDOC_THEME_DIR / "background.webp", ASSETS_DIR / "background.webp")
    for icon in ALERT_ICONS:
        shutil.copy(PDOC_RESOURCES_DIR / icon, ASSETS_DIR / "icons" / icon)


def main() -> None:
    pages = get_pages()
    for page in pages:
        if not page.source.exists():
            print(
                f"error: {page.source.relative_to(ROOT)} does not exist",
                file=sys.stderr,
            )
            sys.exit(1)

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    copy_assets()
    template = Template((THEME_DIR / "page.html").read_text(encoding="utf-8"))
    errors: list[str] = []
    for page in pages:
        render_page(page, pages, template, errors)

    # report all broken links at once instead of failing on the first one
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
