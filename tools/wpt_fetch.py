"""Fetch the web-platform-tests the Rust engine's reftest harness runs.

A blob-filtered, sparse checkout of one pinned WPT commit: only the CSS directories
bindings/rust/blenderui/tests/wpt measures, plus the shared files their tests link
(the Ahem font, /css/support, /css/reference, /images). Run again after changing
WPT_COMMIT or DIRS: it moves the checkout to the new commit / pattern list.

    py -3 tools/wpt_fetch.py            # into .wpt/ at the repository root
    py -3 tools/wpt_fetch.py --dir D    # elsewhere (then set WPT_DIR=D for the tests)
"""

import argparse
import pathlib
import subprocess
import sys

REPO = "https://github.com/web-platform-tests/wpt"
# the WPT commit the expectations (tests/wpt/expectations.txt) were measured at
WPT_COMMIT = "0936286d874b7d5ddcb68ecf47509995c953d750"

# the test directories (bindings/rust/blenderui/tests/wpt/main.rs runs exactly these)
DIRS = [
    "css/css-align",
    "css/css-anchor-position",
    "css/css-backgrounds",
    "css/css-box",
    "css/css-cascade",
    "css/css-color",
    "css/css-display",
    "css/css-flexbox",
    "css/css-grid",
    "css/css-position",
    "css/css-sizing",
    "css/css-text",
    "css/css-values",
    "css/selectors",
]

# files the tests link from outside their directory
SHARED = [
    "/fonts/Ahem.ttf",
    "/fonts/ahem.css",
    # the other fonts tests load (WOFF and plain OpenType; WOFF 2 isn't read yet)
    "/fonts/mplus-1p-regular.woff",
    "/fonts/DoulosSIL-R.woff",
    "/fonts/GentiumPlus-R.woff",
    "/fonts/LinLibertine_Re-4.7.5.woff",
    "/fonts/noto/",
    "/fonts/CSSFWOrientationTest.otf",
    "/fonts/mark-anchor-test.ttf",
    "/css/css-text/resources/",
    "/css/css-fonts/resources/",
    "/css/support/",
    "/css/reference/",
    "/images/",
]

ROOT = pathlib.Path(__file__).resolve().parent.parent


def git(cwd, *args):
    subprocess.run(["git", *args], cwd=cwd, check=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dir", default=str(ROOT / ".wpt"), help="checkout directory (default: .wpt/)")
    out = pathlib.Path(ap.parse_args().dir)
    if not (out / ".git").exists():
        out.mkdir(parents=True, exist_ok=True)
        git(out, "init", "-q")
        git(out, "remote", "add", "origin", REPO)
        git(out, "config", "core.autocrlf", "false")  # the tests' bytes, as WPT has them
    # non-cone patterns: single shared files alongside whole directories
    patterns = ["/" + d + "/" for d in DIRS] + SHARED
    git(out, "sparse-checkout", "set", "--no-cone", *patterns)
    git(out, "fetch", "-q", "--depth=1", "--filter=blob:none", "origin", WPT_COMMIT)
    git(out, "checkout", "-q", "--detach", WPT_COMMIT)
    print(f"WPT {WPT_COMMIT[:12]} in {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
