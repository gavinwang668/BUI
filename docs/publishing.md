# Packaging and publishing (`blenderui` on PyPI)

| | |
|---|---|
| PyPI name | **`blenderui`** (`pip install blenderui`) |
| Import names | `blenderui` (recommended) and `bui` (the same modules), plus the extension `buipy` |
| Build backend | [scikit-build-core](https://scikit-build-core.readthedocs.io) driving the project's CMake (`pyproject.toml`) |
| Version | `bindings/python/bui/_version.py`, the single source (CMake reads it too) |
| Python | 3.11+ (CPython) |
| Console script | `blenderui-tk app.py`, which is the same as `python -m bui.tksyntax app.py` |

> The import name `bui` is kept for existing code. An unrelated project owns
> `bui` on PyPI, so published code should use `import blenderui`. The two
> can't both be installed under the same import name.

## What a wheel contains

```
site-packages/
  buipy.<abi>.pyd|.so     the C kit + raster engine (one extension module)
  bui/                     tksyntax/, qtsyntax.py, components/, host.py ...
  blenderui/               alias package: blenderui.X is bui.X
  blenderui-<v>.dist-info/
```

On Windows the extension is built with MinGW-w64 GCC (UCRT) and statically
linked against the GCC runtime. It depends only on system DLLs and
`python3xx.dll`. A wheel is about 0.4 MB compressed; the sdist is about
1.7 MB (it excludes `reference/` and build trees).

## Build locally

```sh
python -m pip install build
python -m build                    # sdist + wheel in dist/

# Windows with MSYS2 MinGW (the tested toolchain):
set CMAKE_GENERATOR=MinGW Makefiles
set CC=gcc
set CXX=g++
python -m build
```

Test the wheel in a clean environment:

```sh
python -m venv /tmp/bui-venv
/tmp/bui-venv/bin/pip install dist/blenderui-*.whl
/tmp/bui-venv/bin/python -c "import blenderui, blenderui.tksyntax, blenderui.components; print(blenderui.__version__)"
```

The test suites (`bindings/python/tests/`) pass against an installed wheel.
Remove the source tree from `sys.path` and keep `examples/` on it for
`test_components`.

## CI (`.github/workflows/wheels.yml`)

| Job | Does |
|---|---|
| `wheels` | `cibuildwheel` for Linux (manylinux x86_64), Windows (MinGW, x86_64) and macOS (arm64 + x86_64), CPython 3.11–3.14; each wheel is imported as a smoke test |
| `sdist` | `python -m build --sdist` |
| `tests` | CMake build + `ctest` (all C and Python suites) on Windows |
| `publish` | On a `v*` tag: upload everything to PyPI with trusted publishing. Manual run with `testpypi`: upload to TestPyPI. |

## One-time PyPI setup

1. Create the `blenderui` project on PyPI. Register it as a
   [pending trusted publisher](https://docs.pypi.org/trusted-publishers/)
   with this repository, workflow `wheels.yml`, environment `pypi`.
2. Do the same on TestPyPI with environment `testpypi`.
3. In the GitHub repository settings, create the environments `pypi` and
   `testpypi` (optionally with required reviewers).

`pyproject.toml` `[project.urls]` already points at
`github.com/ppmpreetham/BUI`.

No API tokens are stored anywhere.

## NuGet (`BlenderUI`)

The C# binding loads the kit as one shared library. `release.yml` builds it once per platform
(`nuget-native`: linux-x64 on Ubuntu 22.04, osx-arm64, osx-x64, win-x64 with MSVC), then the
`nuget` job packs it as `runtimes/<rid>/native/` (the folder .NET takes native assets from) and
pushes the package with `--skip-duplicate`. On Windows the export list of `bui_kit.dll` is
generated from the functions the binding imports (`bindings/csharp/CMakeLists.txt`).

One-time setup:

1. On nuget.org, open the user menu > **Trusted Publishing** and add a policy: repository owner
   and name of this repository, workflow file `release.yml` (the file name only).
2. In the GitHub repository, add the secret `NUGET_USER`: the nuget.org profile name (not the
   e-mail address).

The policy is bound to the repository's GitHub IDs at the first successful push. A policy of a
private repository stays active for 7 days only until that first push. Each run trades the job's
OIDC token for an API key that is valid for one hour.

## Releasing everything at once

One command does the whole release:

```sh
python tools/release.py minor              # or patch, major, X.Y.Z; add --npm patch for the npm packages
```

It checks you are on a clean, up-to-date `main`, bumps every version and pushes that commit, waits
for the `ci` and `wheels` workflows to go green on it, tags and pushes the tag, waits for the
publish, then checks crates.io, npm, LuaRocks and PyPI for the new version. It never tags a red commit.
`--dry-run` shows the plan; if a publish step fails, fix it, use "Re-run failed jobs" on the run
(no new tag needed) and run `python tools/release.py --resume`, which only waits and checks.
GitHub's anonymous API allows 60 requests an hour (enough for one release); set `GITHUB_TOKEN`
for more.

The two steps it runs, if you want them by hand: `tools/bump_version.py` changes every version in one go
(the Python package, the Rust workspace and the sibling dependency versions of every crate, the LuaRocks rockspec
(renamed to the new version), the DUB package and its `kit.d` version constants, the changelog, the lock files, and
with `--npm` the npm packages) and runs `tools/check_versions.py` on the result:

```sh
python tools/bump_version.py minor --commit --push   # 0.2.0 -> 0.3.0, commits only those files, pushes main
# ... wait for CI to go green on that commit ...
python tools/bump_version.py --tag --push            # tags v0.3.0 and pushes the tag: the release starts
```

`minor` can be `major`, `patch` or an exact `X.Y.Z`; add `--npm patch` to move the npm
packages too (they are versioned on their own), and `--dry-run` to see what would change. The
tag step refuses a tag that already exists and a branch that isn't pushed yet. Doing it by hand
is the same thing: `git tag v0.3.0 && git push origin v0.3.0`.

| Workflow | Publishes |
|---|---|
| `wheels.yml` | the Python wheels and sdist to PyPI (trusted publishing, no secret) |
| `release.yml` | the Rust crates to crates.io, `@blender-ui/native` and `@blender-ui/react` to npm, and a GitHub Release |

`release.yml` first checks that the tag matches the Python, Rust and D versions
(`python tools/check_versions.py v0.2.0` runs the same check by hand; the C header's
`BUI_KIT_VERSION_*` macros and the DUB package's copies are part of it) and that
`THIRD_PARTY_NOTICES.md` is up to date
(`python tools/gen_notices.py` regenerates it when a dependency changes), then runs the whole
`ci.yml` suite on every platform, then publishes. A version that is already on a registry is
skipped (crates.io, npm and PyPI alike: `wheels.yml` builds nothing when PyPI already has the
version), so only what is missing runs: a run that failed halfway (say, crates.io timing out on the big
`blenderui-sys` upload) can simply be re-run. The npm packages are versioned on their own; bump
them in their `package.json` when they change.

One-time setup:

- **crates.io:** a repository secret `CARGO_REGISTRY_TOKEN` (Settings > Secrets and variables >
  Actions): a crates.io API token with the scopes `publish-new` and `publish-update`.
- **npm:** no secret. On npmjs.com, open each of `@blender-ui/native` and `@blender-ui/react`
  > Settings > Trusted Publisher > GitHub Actions, and enter the owner `ppmpreetham`, the
  repository `BUI` and the workflow filename `release.yml` (leave the environment empty).
  The workflow publishes with OIDC and npm adds provenance by itself.
- **A new npm package** (a new framework binding, such as `@blender-ui/svelte`) can't have a trusted
  publisher before it exists, and a first `npm publish` from CI has no permission yet (the 404
  "could not be found or you do not have permission"). Publish it once by hand, after
  `@blender-ui/native` is out (it depends on it): `cd bindings/svelte && npm publish --access public`,
  logged in as a member of the `@blender-ui` organization. Then add the trusted publisher (the same
  form as above). The same goes for `@blender-ui/vue` (`cd bindings/vue && npm publish --access public`). Until then `release.yml` skips it with a warning and `tools/release.py` shows it
  as MANUAL, not as a failure.
- **LuaRocks:** a repository secret `LUAROCKS_API_KEY`: on luarocks.org, log in with the account that
  owns the package (the repository owner's name), Settings > API keys > create one. The `luarocks` job uploads
  `bindings/lua/blenderui-<version>-1.rockspec`; the rock itself is built from the tag by whoever runs
  `luarocks install blenderui` (it needs CMake and a C compiler). The first upload creates the package.
- **PyPI:** no secret, see above.
- **RubyGems:** not in `release.yml` yet: the gem is published by hand. `cd bindings/ruby && rake gem`
  copies the kit's C sources into `ext/bui/csrc/` (`vendor_c.py --dest`, which empties only a folder it
  made, marked `.vendored`, or an empty one, and refuses any other) and builds
  `blenderui-<version>.gem`, which builds the kit wherever it is installed (it needs CMake and a C and
  C++20 compiler); `gem push blenderui-<version>.gem`, logged in as the gem's owner. The version is
  `bindings/ruby/lib/bui/version.rb`, which `tools/bump_version.py` moves with the others.

Try it without publishing: Actions > release > Run workflow, enter the tag and leave
"publish" unticked. That runs the checks and the CI suite, then `cargo publish --dry-run` and
`npm publish --dry-run`.

## Accounts and one-time setup for the first publish

Everything below is done once; after that a tag publishes by itself. A job whose secret is missing
builds its package and warns instead of failing, so a release is never blocked by an account you have
not made yet (a missing registry *account* is not an error; a missing *token* on crates.io is).

| Where | Account and setup | Secret | What the tag then does |
|---|---|---|---|
| **PyPI** | Done (`blenderui`). | none (trusted publishing) | wheels and sdist |
| **crates.io** | Done. A new crate (`bui-a11y` is new) needs the token's `publish-new` scope. | `CARGO_REGISTRY_TOKEN` | nine crates in order |
| **npm** | Done for `native`, `react`, `svelte` and the platform packages. **`@blender-ui/vue` is not on npm**: after `@blender-ui/native` of the release is out, `cd bindings/vue && npm publish --access public` once by hand (logged in as a member of the `@blender-ui` organization), then add its trusted publisher (same form as the others, workflow `release.yml`). | none | the packages |
| **LuaRocks** | Done. | `LUAROCKS_API_KEY` | uploads the rockspec |
| **NuGet** (`BlenderUI`) | Make an account on nuget.org (a Microsoft account). User menu > **Trusted Publishing** > add a policy: owner `ppmpreetham`, repository `BUI`, workflow file `release.yml`. Scopes: **Push**, and choose "Push new packages and package versions" (the first push creates the package). Glob patterns: `BlenderUI`. Environment: empty. The package id `BlenderUI` is yours from its first push. | `NUGET_USER`: your nuget.org profile name (not the e-mail) | packs, then pushes |
| **RubyGems** (`blenderui`) | Make an account on rubygems.org and turn on multi-factor authentication. Profile > **Pending trusted publishers** > new: RubyGem name **`blenderui`** (exactly the gemspec's name, lowercase), publisher type GitHub Actions, owner `ppmpreetham`, repository `BUI`, workflow filename **`release.yml`**, environment empty. The page shows your account's id: save it as the repository **variable** `RUBYGEMS_OWNER_ID` (Settings > Secrets and variables > Actions > Variables). The first push creates the gem and the publisher stays attached to it. | variable `RUBYGEMS_OWNER_ID`, no secret | builds the gem (the kit's C sources inside) and pushes |
| **Go** | No account. The `go-tag` job pushes the tag `bindings/go/vX.Y.Z` that Go needs to find a module that lives in a folder; pkg.go.dev picks the module up by itself the first time someone fetches it. | none (the job's own token) | tags |
| **DUB** (`blenderui`) | Once: log in to code.dlang.org with GitHub, then **Register a package**: GitHub, owner `ppmpreetham`, repository `BUI`. The name `blenderui` comes from the root `dub.json` and was free when this was written. Do it before or after the tag; the registry reads the repository by itself. | none | nothing in CI: the registry watches the repository and publishes every new `v*` tag about twice an hour |
| **GitHub Release** | Done. `cli.yml` attaches the `bui` archives to the same release. | none | creates the release |

Set a secret in the repository: Settings > Secrets and variables > Actions > New repository secret.

**How the DUB package is laid out.** code.dlang.org wants the recipe at the repository root and serves the
repository as it is at each tag. The root `dub.json` is a thin recipe that only hands over to the real one
(`"subPackages": ["./bindings/d"]`, depending on `blenderui:blenderui`), so there is one recipe to keep up to date:
`bindings/d/dub.json`, whose version `tools/bump_version.py` moves. A user of `dub add blenderui` gets the whole
repository at that tag (the C kit is built from its sources by `bindings/d/scripts/build_bui.*`), so the
download is large. Only tags that have the root `dub.json` (0.6.0 onward) can be served. Check a release with a
clean folder: `dub add-local <checkout> 0.6.0`, a project that depends on `"blenderui": "~>0.6.0"`, `dub build`.

**Not published by the release (do by hand, or leave out):**

- **`bui-charts` and `bui-panels`:** not on crates.io. They are not in the release's crate list.
- **The editor extensions:** VS Code (`npx @vscode/vsce package`, publisher `ppmpreetham`, then `vsce publish`
  with a Marketplace token; Open VSX takes the same file with its own token) and Zed (a pull request to
  the `zed-industries/extensions` repository). They are versioned on their own.

## Checks that do not hold back a release

`ci.yml` is the gate: `release.yml` runs it before anything is published. `extras.yml` runs beside it and
is never waited for: the JavaScript packages' own tests (`npm test`), the website build with a check that
no internal link is broken (`tools/check_site_links.py`), `cargo fmt` and a count of clippy's warnings, and
that the demos and the addons still compile. A red job there is something to fix, not a stopped release.
Move a job into `ci.yml` once it has been green for a while.

## Release checklist

1. Bump `bindings/python/bui/_version.py` (semantic versioning).
2. Add the release to `CHANGELOG.md`.
3. `ctest --test-dir build` passes locally. Optionally run the workflow by
   hand with `testpypi` and `pip install -i https://test.pypi.org/simple blenderui`.
4. `git tag v0.2.0 && git push --tags`. CI builds, tests and publishes (see above).

## Known gaps before 1.0

- Linux wheels get the native X11 window host: `[tool.cibuildwheel.linux]
  before-all` installs the X11 headers (`libX11-devel` / `libx11-dev`), and
  libX11 itself is loaded at run time, so the wheels still import on servers
  without it. (This used to be a gap; it's fixed.)
- macOS: the native Cocoa window host is new and has been tried by hand on
  three Macs (two Apple Silicon, one Intel) so far. With `BUI_HOST=tk` the wheels use the stdlib Tk window instead, which
  needs `tkinter` (for example from python.org or `brew install python-tk`).
- MSVC builds work (the Rust crate and the CMake build use Visual Studio's
  toolchain), but the published Windows wheels are built with MinGW.
- No `musllinux` or 32-bit wheels yet (skipped in `[tool.cibuildwheel]`).
- The README's images use absolute `raw.githubusercontent.com` URLs, so
  PyPI shows them.

# Publishing the Rust crates (crates.io)

`blenderui` is on [crates.io](https://crates.io/crates/blenderui)
(`cargo add blenderui`). The workspace in `bindings/rust/` has thirteen crates
(`bui-a11y` joins it from `platform/a11y/`). A `v*` tag publishes nine of them to crates.io, in dependency order (see
above); the others stay in the repository:

| Crate | What it is | Depends on | crates.io |
|---|---|---|---|
| `blenderui-tailwind` | Tailwind v4 class compiler (std only) | none | Published |
| `blenderui-macros` | `rsx!` / `view!`, `#[component]`, `css!`, `tw!` | `blenderui-tailwind` | Published |
| `bui-a11y` | screen readers: the kit's accessibility trees as AccessKit trees (lives in `platform/a11y/`, a member of this workspace) | none | Published |
| `blenderui-sys` | FFI over the C kit; builds it with CMake | `bui-a11y` (`a11y` feature; `cmake` at build time) | Published |
| `blenderui-paint` | shared painting types: geometry, colors, `PaintSink`, the text cache | none | Published |
| `blenderui-table` | the data table's core | `blenderui-paint` | Published |
| `blenderui-vlist` | the variable-height list's core | `blenderui-paint` | Published |
| `blenderui` | the framework | the five above | Published |
| `blenderui-imgui` | Dear ImGui on BUI (`blender-widgets` feature for Blender's widgets) | `blenderui-sys` (`imgui` feature) | Published |
| `blenderui-jsview` | The JavaScript view of a window, shared by the Node.js addon and the QuickJS runtime | `blenderui` | `publish = false` |
| `blenderui-node` | The napi-rs addon behind `@blender-ui/native` | `blenderui`, `blenderui-jsview` | `publish = false` (ships on npm) |
| `blenderui-qjs` | `bui-runtime`: BUI with embedded QuickJS, for `bui build` | `blenderui`, `blenderui-jsview` | `publish = false` (ships on npm) |
| `blenderui-web` | A blenderui app in a web page (`wasm32-wasip1`, `js/bui-web.js`) | `blenderui`, `blenderui-sys` | Not published by `release.yml` |

`bui-a11y` (screen readers through AccessKit) lives in `platform/a11y/` but is a member of
this workspace, so it shares the version and lock. `blenderui-sys` depends on it
with the `a11y` feature (it is published first), and the C kit's CMake build builds
it with cargo. It is new in 0.5.x: until it is on crates.io, package `blenderui-sys`
together with it.

`blenderui-sys` must carry the C sources, because a published crate can't
reach `../../..`. Vendor them first. They're git-ignored and listed in the
crate's `include`:

```sh
python bindings/rust/scripts/vendor_c.py          # copies ~7 MB into blenderui-sys/csrc/
cd bindings/rust
cargo package --workspace --allow-dirty           # builds each packaged crate in isolation
# blenderui-sys alone needs bui-a11y with it until that version is on crates.io:
# cargo package -p bui-a11y -p blenderui-sys --allow-dirty
```

`build.rs` looks for the C sources in three places, in order:
1. `BUI_SOURCE_DIR`, if set;
2. the surrounding repository;
3. `csrc/`, the vendored copy (what a downloaded crate has).

## Release

1. Bump `version` in `bindings/rust/Cargo.toml` (`[workspace.package]`). The
   crates share one version. Then update the `version = "..."` of the path
   dependencies in `blenderui/Cargo.toml`, `blenderui-macros/Cargo.toml`,
   `blenderui-imgui/Cargo.toml`, `blenderui-table/Cargo.toml`,
   `blenderui-vlist/Cargo.toml` and `blenderui-web/Cargo.toml`
   (`python tools/bump_version.py` does all of them).
2. `cargo login` once with a crates.io API token.
3. Vendor, then publish in order. `cargo publish --workspace` does the
   ordering itself on Cargo 1.90+:

```sh
python bindings/rust/scripts/vendor_c.py
cd bindings/rust
cargo publish --workspace --allow-dirty   # or, one by one:
# cargo publish -p blenderui-tailwind
# cargo publish -p blenderui-macros
# cargo publish -p bui-a11y
# cargo publish -p blenderui-sys --allow-dirty
# cargo publish -p blenderui-paint
# cargo publish -p blenderui-table
# cargo publish -p blenderui-vlist
# cargo publish -p blenderui
# cargo publish -p blenderui-imgui
```

`cargo publish --workspace` skips the `publish = false` crates, but not
`blenderui-web`. `release.yml` publishes the nine crates one by one, in the
order above.

`--allow-dirty` is needed because `csrc/` is untracked. Users need CMake and
a C compiler (MSVC, MinGW, gcc or clang) to build `blenderui-sys`.
`bench/` is excluded from the workspace and is never published.
