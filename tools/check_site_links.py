#!/usr/bin/env python3
"""Check the internal links of the built website (website/dist).

    cd website && bun run build && cd .. && python tools/check_site_links.py

Every `href="/..."` of every page must name a file or a folder with an index.html in `dist/`. Anchors
(`#...`) and query strings are ignored. Exits 1 and lists the broken links (with a page that has each)
if there are any. `--dist DIR` checks another folder.

SPDX-License-Identifier: GPL-2.0-or-later
"""

import argparse
import re
import sys
from pathlib import Path

HREF = re.compile(r'href="(/[^"#?]*)')


def exists(dist: Path, link: str) -> bool:
    target = dist / link.lstrip("/")
    return (
        (target.is_file())
        or (target / "index.html").is_file()
        or target.with_name(target.name + ".html").is_file()
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--dist", default=str(Path(__file__).resolve().parent.parent / "website" / "dist"))
    dist = Path(ap.parse_args().dist)
    if not dist.is_dir():
        print(f"{dist} does not exist: build the website first (cd website && bun run build)", file=sys.stderr)
        return 2

    broken: dict[str, str] = {}
    pages = 0
    for page in dist.rglob("*.html"):
        pages += 1
        text = page.read_text(encoding="utf-8", errors="ignore")
        for link in set(HREF.findall(text)):
            if not exists(dist, link):
                broken.setdefault(link, page.relative_to(dist).as_posix())

    for link, page in sorted(broken.items()):
        print(f"{link}  (on /{page})")
    print(f"{pages} pages checked, {len(broken)} broken links")
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main())
