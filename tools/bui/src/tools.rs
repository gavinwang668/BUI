//! The programs a language needs (Rust's cargo, Bun, uv, ...): whether they are there, the commands
//! that install the newest one on this machine, and the question that offers to run them.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

use crate::sys::{Cmd, OS, Os, semver, version_of, which};
use crate::ui;

static INSTALL_WITHOUT_ASKING: AtomicBool = AtomicBool::new(false);

/// `--install-tools`: install what is missing without the question (scripts, CI).
pub fn install_without_asking() {
    INSTALL_WITHOUT_ASKING.store(true, Ordering::Relaxed);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Rust,
    Bun,
    Node,
    Pnpm,
    Uv,
    Lua,
    Luarocks,
    Cmake,
    /// A C and a C++ compiler (on Windows, Visual Studio's C++ tools).
    Cc,
    Git,
    /// Go 1.22 or newer.
    Go,
    /// gcc and g++ (on Windows MinGW-w64's: cgo cannot use Visual Studio's compiler).
    Gcc,
    /// The X11 development headers (Linux; there is nothing to check elsewhere).
    X11,
    /// The .NET SDK, 8 or newer.
    Dotnet,
    Dub,
    /// LDC, the D compiler.
    Ldc,
    /// Ruby 3.1 or newer.
    Ruby,
    Bundler,
}

/// How to install a tool: commands, in order.
pub struct Install {
    pub cmds: Vec<Cmd>,
    pub note: Option<&'static str>,
}

fn pkg(apt: &str, dnf: &str, pacman: &str) -> Cmd {
    let line = if which("apt-get").is_some() {
        format!("sudo apt-get install -y {apt}")
    } else if which("dnf").is_some() {
        format!("sudo dnf install -y {dnf}")
    } else if which("pacman").is_some() {
        format!("sudo pacman -S --needed --noconfirm {pacman}")
    } else {
        format!("sudo apt-get install -y {apt}")
    };
    Cmd::script(&line)
}

fn winget(id: &str) -> Cmd {
    Cmd::new("winget", ["install", "-e", "--id", id, "--accept-source-agreements", "--accept-package-agreements"])
}

fn brew(formulae: &str) -> Cmd {
    Cmd::script(&format!("brew install {formulae}"))
}

impl Tool {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust (cargo)",
            Self::Bun => "Bun",
            Self::Node => "Node.js",
            Self::Pnpm => "pnpm",
            Self::Uv => "uv",
            Self::Lua => "Lua 5.4",
            Self::Luarocks => "LuaRocks",
            Self::Cmake => "CMake",
            Self::Cc => "a C and C++ compiler",
            Self::Git => "Git",
            Self::Go => "Go 1.22+",
            Self::Gcc => "gcc and g++",
            Self::X11 => "X11 development headers",
            Self::Dotnet => ".NET 8 SDK",
            Self::Dub => "DUB",
            Self::Ldc => "LDC (D compiler)",
            Self::Ruby => "Ruby 3.1+",
            Self::Bundler => "Bundler",
        }
    }

    /// The version when it is installed (`"found"` when it has none to print).
    pub fn probe(self) -> Option<String> {
        let v = |program: &str, arg: &str| which(program).map(|_| version_of(program, &[arg]).unwrap_or_else(|| "found".into()));
        match self {
            Self::Rust => v("cargo", "--version"),
            Self::Bun => v("bun", "--version"),
            Self::Node => v("node", "--version"),
            Self::Pnpm => v("pnpm", "--version"),
            Self::Uv => v("uv", "--version"),
            Self::Lua => ["lua5.4", "lua54", "lua"]
                .into_iter()
                .find_map(|p| which(p).and_then(|_| version_of(p, &["-v"]).filter(|ver| ver.starts_with("5.4")))),
            Self::Luarocks => v("luarocks", "--version"),
            Self::Cmake => v("cmake", "--version"),
            Self::Git => v("git", "--version"),
            Self::Cc => cc_probe(),
            Self::Go => v("go", "version").filter(|ver| at_least(ver, [1, 22])),
            Self::Gcc => which("g++").and_then(|_| v("gcc", "--version")),
            Self::X11 => x11_probe(),
            Self::Dotnet => v("dotnet", "--version").filter(|ver| at_least(ver, [8, 0])),
            Self::Dub => v("dub", "--version"),
            Self::Ldc => v("ldc2", "--version"),
            Self::Ruby => v("ruby", "--version").filter(|ver| at_least(ver, [3, 1])),
            Self::Bundler => v("bundle", "--version"),
        }
    }

    pub fn install(self) -> Install {
        let none = |cmds: Vec<Cmd>| Install { cmds, note: None };
        match (self, OS) {
            (Self::Rust, Os::Windows) => Install {
                cmds: vec![winget("Rustlang.Rustup")],
                note: Some("Rust on Windows also needs Visual Studio's C++ build tools (bui offers them next)."),
            },
            (Self::Rust, _) => none(vec![Cmd::script("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y")]),
            (Self::Bun, Os::Windows) => none(vec![Cmd::script("irm https://bun.sh/install.ps1 | iex")]),
            (Self::Bun, _) => none(vec![Cmd::script("curl -fsSL https://bun.sh/install | bash")]),
            (Self::Node, Os::Windows) => none(vec![winget("OpenJS.NodeJS.LTS")]),
            (Self::Node, Os::Mac) if which("brew").is_some() => none(vec![brew("node")]),
            (Self::Node, _) => none(vec![Cmd::script(
                "curl -fsSL https://fnm.vercel.app/install | bash && export PATH=\"$HOME/.local/share/fnm:$PATH\" && eval \"$(fnm env)\" && fnm install --lts && fnm default lts-latest",
            )]),
            (Self::Pnpm, _) => none(vec![Cmd::new("npm", ["install", "--global", "pnpm"])]),
            (Self::Uv, Os::Windows) => none(vec![Cmd::script("irm https://astral.sh/uv/install.ps1 | iex")]),
            (Self::Uv, _) => none(vec![Cmd::script("curl -LsSf https://astral.sh/uv/install.sh | sh")]),
            (Self::Lua | Self::Luarocks, Os::Windows) => Install {
                cmds: vec![Cmd::script("scoop install lua luarocks")],
                note: Some("Needs Scoop (https://scoop.sh): `irm https://get.scoop.sh | iex`."),
            },
            (Self::Lua | Self::Luarocks, Os::Mac) => none(vec![brew("lua luarocks")]),
            (Self::Lua | Self::Luarocks, Os::Linux) => none(vec![pkg(
                "lua5.4 liblua5.4-dev luarocks build-essential",
                "lua lua-devel luarocks gcc make",
                "lua luarocks base-devel",
            )]),
            (Self::Cmake, Os::Windows) => none(vec![winget("Kitware.CMake")]),
            (Self::Cmake, Os::Mac) => none(vec![brew("cmake")]),
            (Self::Cmake, Os::Linux) => none(vec![pkg("cmake", "cmake", "cmake")]),
            (Self::Cc, Os::Windows) => Install {
                cmds: vec![Cmd::new(
                    "winget",
                    [
                        "install", "-e", "--id", "Microsoft.VisualStudio.2022.BuildTools", "--override",
                        "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended",
                    ],
                )],
                note: Some("A large download (a few GB); it only has to happen once."),
            },
            (Self::Cc | Self::Gcc | Self::Git, Os::Mac) => none(vec![Cmd::script("xcode-select --install")]),
            (Self::Cc | Self::Gcc, Os::Linux) => none(vec![pkg("build-essential", "gcc gcc-c++ make", "base-devel")]),
            (Self::Gcc, Os::Windows) => Install {
                cmds: vec![winget("BrechtSanders.WinLibs.POSIX.UCRT")],
                note: Some("MinGW-w64's gcc, for cgo (MSVC does not work with it). Open a new terminal afterwards so its folder is on PATH."),
            },
            (Self::Git, Os::Windows) => none(vec![winget("Git.Git")]),
            (Self::Git, Os::Linux) => none(vec![pkg("git", "git", "git")]),
            (Self::Go, Os::Windows) => none(vec![winget("GoLang.Go")]),
            (Self::Go, Os::Mac) => none(vec![brew("go")]),
            (Self::Go, Os::Linux) => none(vec![pkg("golang", "golang", "go")]),
            (Self::X11, _) => none(vec![pkg("libx11-dev", "libX11-devel", "libx11")]),
            (Self::Dotnet, Os::Windows) => none(vec![winget("Microsoft.DotNet.SDK.8")]),
            (Self::Dotnet, Os::Mac) => none(vec![brew("--cask dotnet-sdk")]),
            (Self::Dotnet, Os::Linux) => none(vec![pkg("dotnet-sdk-8.0", "dotnet-sdk-8.0", "dotnet-sdk-8.0")]),
            (Self::Dub | Self::Ldc, Os::Windows) => Install {
                cmds: vec![Cmd::script("scoop install ldc dub")],
                note: Some("Needs Scoop (https://scoop.sh): `irm https://get.scoop.sh | iex`."),
            },
            (Self::Dub | Self::Ldc, Os::Mac) => none(vec![brew("ldc dub")]),
            (Self::Dub | Self::Ldc, Os::Linux) => none(vec![pkg("ldc dub", "ldc dub", "ldc dub")]),
            (Self::Ruby, Os::Windows) => Install {
                cmds: vec![winget("RubyInstallerTeam.RubyWithDevKit.3.4")],
                note: Some("RubyInstaller with its MSYS2 development kit, which has the compilers the gem needs."),
            },
            (Self::Ruby, Os::Mac) => none(vec![brew("ruby")]),
            (Self::Ruby, Os::Linux) => none(vec![pkg("ruby-full", "ruby ruby-devel", "ruby")]),
            (Self::Bundler, _) => none(vec![Cmd::new("gem", ["install", "bundler"])]),
        }
    }
}

/// Whether the dotted version `ver` (`1.22.1`) is `min` (major, minor) or newer.
fn at_least(ver: &str, min: [u64; 2]) -> bool {
    let [major, minor, _] = semver(ver);
    [major, minor] >= min
}

/// The X11 headers on Linux (`Xlib.h` where the system or `pkg-config` keeps it); other systems have nothing to check.
fn x11_probe() -> Option<String> {
    if OS != Os::Linux {
        return Some("not needed here".to_owned());
    }
    let header = std::path::Path::new("/usr/include/X11/Xlib.h").is_file();
    (header || Cmd::new("pkg-config", ["--exists", "x11"]).capture().is_some()).then(|| "found".to_owned())
}

/// The X11 headers, for the languages that compile BUI's windows against them: on Linux only.
pub const fn x11_headers() -> Option<Tool> {
    if matches!(OS, Os::Linux) { Some(Tool::X11) } else { None }
}

fn cc_probe() -> Option<String> {
    if OS == Os::Windows {
        let root = std::env::var_os("ProgramFiles(x86)")?;
        let vswhere = std::path::Path::new(&root).join("Microsoft Visual Studio/Installer/vswhere.exe");
        let out = Cmd::new(
            vswhere.to_str()?,
            [
                "-latest", "-products", "*", "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property", "installationPath",
            ],
        )
        .capture()?;
        let path = out.trim();
        return (!path.is_empty()).then(|| "Visual Studio C++ tools".to_owned());
    }
    // BUI's kit is C++: every flavour that builds it (C, Rust, Lua) needs both compilers
    let found = |names: [&'static str; 3]| names.into_iter().find(|c| which(c).is_some());
    Some(format!("{} and {}", found(["cc", "gcc", "clang"])?, found(["c++", "g++", "clang++"])?))
}

/// Looks for each tool; for the missing ones shows how to install the newest, offers to run it and
/// looks again. Returns the tools that are still missing.
pub fn ensure(tools: &[Tool]) -> Result<Vec<Tool>> {
    let mut wanted: Vec<Tool> = Vec::new();
    for t in tools {
        if !wanted.contains(t) {
            wanted.push(*t);
        }
    }
    let missing: Vec<Tool> = wanted.into_iter().filter(|t| t.probe().is_none()).collect();
    if missing.is_empty() {
        return Ok(missing);
    }
    let names: Vec<&str> = missing.iter().map(|t| t.label()).collect();
    ui::warn(format!("Not installed: {}", names.join(", ")));
    let mut done: Vec<String> = Vec::new();
    for tool in &missing {
        let install = tool.install();
        let shown: Vec<String> = install.cmds.iter().map(Cmd::display).collect();
        if shown.iter().all(|s| done.contains(s)) {
            continue; // the same command already installed this one (Lua and LuaRocks)
        }
        let mut body = shown.iter().map(|s| format!("$ {}", ui::code(s))).collect::<Vec<_>>().join("\n");
        if let Some(n) = install.note {
            body = format!("{body}\n{}", ui::dim(n));
        }
        ui::note(&format!("Install the newest {}", tool.label()), &body);
        let go = INSTALL_WITHOUT_ASKING.load(Ordering::Relaxed)
            || (ui::asking() && ui::confirm(&format!("Run it now to install {}?", tool.label()), true)?);
        if !go {
            continue;
        }
        for cmd in &install.cmds {
            ui::flush();
            if !cmd.run_inherit().is_ok_and(|s| s.success()) {
                ui::error(format!("Could not install {}. Run the command above yourself, then `bui doctor`.", tool.label()));
                break;
            }
        }
        done.extend(shown);
    }
    let still: Vec<Tool> = missing.into_iter().filter(|t| t.probe().is_none()).collect();
    if !still.is_empty() && !done.is_empty() {
        ui::info(format!(
            "Installed, but not visible to this terminal yet: open a new terminal and run {}.",
            ui::code("bui doctor")
        ));
    }
    Ok(still)
}
