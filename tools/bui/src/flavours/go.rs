//! Go: BUI's `bindings/go` module, which needs the C sources next to it, so `bui` keeps one shallow
//! checkout of the newest release (`~/.bui/BUI`) that every Go app points its `replace` at.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{Adopted, File, Flavour, Step, Vars, newest_tag, read, read_manifest, render, step};
use crate::check::{self, Check, Fix, Work};
use crate::registry;
use crate::sys::{Cmd, OS, Os, bui_home};
use crate::tools::{self, Tool};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoApp {}

const MODULE: &str = "github.com/ppmpreetham/BUI/bindings/go";

/// Builds BUI's C kit into the checkout's `.bui/` (what cgo links), once and after the checkout moves.
const GENERATE: &str = "github.com/ppmpreetham/BUI/bindings/go/bui";

/// The one checkout of BUI that every Go app uses.
fn checkout() -> PathBuf {
    bui_home().unwrap_or_else(|| PathBuf::from(".bui")).join("BUI")
}

/// Where the module is inside the checkout, as `go.mod` writes it (forward slashes on every system).
fn module_dir() -> String {
    checkout().join("bindings").join("go").to_string_lossy().replace('\\', "/")
}

fn go(dir: &Path, args: &[&str]) -> Cmd {
    Cmd::new("go", args.iter().copied()).in_dir(dir)
}

fn generate(dir: &Path) -> Cmd {
    go(dir, &["generate", GENERATE])
}

fn git(dir: &Path, args: &[&str]) -> Cmd {
    Cmd::new("git", args.iter().copied()).in_dir(dir)
}

/// The release tag the checkout is at, if it is a checkout of one.
fn checked_out_tag(dir: &Path) -> Option<String> {
    Some(git(dir, &["describe", "--tags", "--exact-match"]).capture()?.trim().to_owned()).filter(|t| !t.is_empty())
}

/// The commands that put the checkout at `tag`: a clone, a fetch of the tag where it is at another, or nothing.
fn to_tag(at: &Path, tag: &str) -> Vec<Cmd> {
    if !at.join(".git").exists() {
        let url = format!("https://github.com/{}", registry::REPO);
        return vec![Cmd::new("git", ["clone".to_owned(), "--quiet".into(), "--depth".into(), "1".into(), "--branch".into(), tag.to_owned(), url, at.to_string_lossy().into_owned()])];
    }
    if checked_out_tag(at).as_deref() == Some(tag) {
        return Vec::new();
    }
    vec![git(at, &["fetch", "--quiet", "--depth", "1", "origin", "tag", tag]), git(at, &["checkout", "--quiet", "--force", tag])]
}

/// The checkout at `tag`, then the kit built, as steps.
fn setup(dir: &Path, tag: &str) -> Vec<Step> {
    let mut steps: Vec<Step> = to_tag(&checkout(), tag).into_iter().map(|c| step(&format!("Fetching BUI {tag} (once, shared by every Go app)"), c)).collect();
    steps.push(step("Building BUI's C kit once (a few minutes now, seconds after)", generate(dir)));
    steps
}

/// The lines `go.mod` lacks to use the module from the checkout (empty when it has them).
fn missing_lines(go_mod: &str) -> String {
    // a line naming the module is its requirement, or (with `=>`) its replacement, alone or inside a block
    let named = || go_mod.lines().filter(|l| l.contains(MODULE));
    let require = (!named().any(|l| !l.contains("=>"))).then(|| format!("\nrequire {MODULE} v0.0.0\n"));
    let replace = (!named().any(|l| l.contains("=>"))).then(|| format!("\nreplace {MODULE} => \"{}\"\n", module_dir()));
    [require, replace].into_iter().flatten().collect()
}

/// A project in Go that uses the module.
pub fn detect(dir: &Path) -> bool {
    read(dir, "go.mod").is_some_and(|t| t.lines().any(|l| l.contains(MODULE)))
}

fn go_mod(name: &str) -> String {
    format!("module {name}\n\ngo 1.22\n{}", missing_lines(""))
}

impl Flavour for GoApp {
    fn label(&self) -> String {
        "Go".into()
    }

    fn tools(&self) -> Vec<Tool> {
        // cgo cannot use Visual Studio's compiler: MinGW-w64's gcc on Windows
        let mut tools = vec![Tool::Go, if OS == Os::Windows { Tool::Gcc } else { Tool::Cc }, Tool::Cmake, Tool::Git];
        tools.extend(tools::x11_headers());
        tools
    }

    /// An existing `go.mod` keeps everything it has; the `require` and `replace` lines for the module are added when absent.
    fn adopt(&self, dir: &Path, _name: &str) -> Result<Vec<Adopted>> {
        let Some(text) = read_manifest(dir, "go.mod")? else {
            return Ok(Vec::new());
        };
        let lines = missing_lines(&text);
        let note = if lines.is_empty() {
            "already uses the BUI module; left as it was".to_owned()
        } else {
            let path = dir.join("go.mod");
            std::fs::write(&path, format!("{}{lines}", text.trim_end_matches('\n').to_owned() + "\n"))?;
            "kept as it was, and added the BUI module (require and replace)".to_owned()
        };
        Ok(vec![Adopted { path: "go.mod", note }])
    }

    fn files(&self, vars: &Vars) -> Vec<File> {
        let mut files = render(&["go"], vars);
        files.push(File { path: "go.mod".into(), body: go_mod(&vars.name) });
        files
    }

    fn deps(&self, dir: &Path) -> Vec<Step> {
        setup(dir, &newest_tag())
    }

    fn dev(&self, dir: &Path, args: &[String]) -> Result<Vec<Cmd>> {
        Ok(vec![go(dir, &["run", "."]).with_args(args)])
    }

    fn build(&self, dir: &Path, args: &[String]) -> Result<Vec<Cmd>> {
        Ok(vec![go(dir, &["build"]).with_args(args)])
    }

    fn checks(&self, dir: &Path) -> Vec<Check> {
        let Some(text) = read(dir, "go.mod") else {
            return vec![Check::fail("go.mod", "missing", None)];
        };
        let lines = missing_lines(&text);
        if !lines.is_empty() {
            let to = format!("{}{lines}", text.trim_end_matches('\n').to_owned() + "\n");
            let fix = Fix::edit("adds the BUI module to go.mod".into(), vec![Work::Rewrite { path: dir.join("go.mod"), at: 0..text.len(), was: text, to }]);
            return vec![Check::fail("BUI module", "go.mod has no require and replace for it", Some(fix))];
        }
        let newest = registry::bui_version().map(|v| format!("v{v}"));
        let tag = newest.clone().unwrap_or_else(newest_tag);
        let at = checkout();
        let fetch = || to_tag(&at, &tag).into_iter().chain([generate(dir)]).collect::<Vec<_>>();
        let Some(have) = at.join(".git").exists().then(|| checked_out_tag(&at)).flatten() else {
            let detail = if at.join(".git").exists() { "not at a release" } else { "not fetched yet" };
            return vec![Check::fail("BUI checkout", format!("{detail}: {}", at.display()), Some(Fix::run("fetches the newest BUI release and builds its kit", fetch())))];
        };
        if !at.join("bindings/go/.bui/lib").is_dir() {
            return vec![Check::fail("BUI kit", "the C kit is not built", Some(Fix::run("builds BUI's C kit", vec![generate(dir)])))];
        }
        let fix = Fix::run("moves the checkout to the newest release and rebuilds the kit", fetch());
        vec![check::newest("BUI checkout", &have, newest.as_deref(), fix)]
    }

    fn update(&self, dir: &Path) -> Vec<Step> {
        registry::bui_version().map(|v| setup(dir, &format!("v{v}"))).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bui-go-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_new_module_points_at_the_checkout() {
        let files = GoApp {}.files(&Vars::new("my-app"));
        let body = |path: &str| files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("{path}")).body.clone();
        let go_mod = body("go.mod");
        assert!(go_mod.starts_with("module my-app\n\ngo 1.22\n"), "{go_mod}");
        assert!(go_mod.contains("require github.com/ppmpreetham/BUI/bindings/go v0.0.0"));
        assert!(go_mod.contains(&format!("replace github.com/ppmpreetham/BUI/bindings/go => \"{}\"", module_dir())), "{go_mod}");
        assert!(module_dir().ends_with("/BUI/bindings/go") && !module_dir().contains('\\'));
        assert!(body("main.go").contains("bindings/go/ui") && body("main.go").contains("\"My App\""));
        assert!(body(".gitignore").contains("/my-app"));
    }

    #[test]
    fn an_existing_go_mod_gets_only_the_missing_lines() {
        let dir = scratch("adopt");
        std::fs::write(dir.join("go.mod"), "module mine\n\ngo 1.23\n\nrequire github.com/x/y v1.0.0\n").unwrap();
        let adopted = GoApp {}.adopt(&dir, "demo").unwrap();
        assert!(adopted[0].note.contains("added"));
        let text = std::fs::read_to_string(dir.join("go.mod")).unwrap();
        assert!(text.starts_with("module mine\n\ngo 1.23\n\nrequire github.com/x/y v1.0.0\n"), "theirs is kept: {text}");
        assert!(text.contains("require github.com/ppmpreetham/BUI/bindings/go v0.0.0") && text.contains("replace "));
        assert!(GoApp {}.adopt(&dir, "demo").unwrap()[0].note.contains("left as it was"));
        assert_eq!(text, std::fs::read_to_string(dir.join("go.mod")).unwrap(), "a second pass changes nothing");
        assert!(detect(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_go_mod_with_the_require_alone_gets_the_replace() {
        let text = "module m\n\nrequire github.com/ppmpreetham/BUI/bindings/go v0.0.0\n";
        let lines = missing_lines(text);
        assert!(lines.contains("replace ") && !lines.contains("require"), "{lines}");
        let both = "module m\nrequire (\n\tgithub.com/ppmpreetham/BUI/bindings/go v0.0.0\n)\nreplace github.com/ppmpreetham/BUI/bindings/go => ../BUI/bindings/go\n";
        assert!(missing_lines(both).is_empty(), "their own replace is kept");
    }

    #[test]
    fn an_unreadable_go_mod_stops_init() {
        let dir = scratch("broken");
        std::fs::write(dir.join("go.mod"), [0xff, 0xfe]).unwrap();
        assert!(GoApp {}.adopt(&dir, "demo").is_err());
        assert!(GoApp {}.adopt(&scratch("none"), "demo").unwrap().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_checkout_is_cloned_shallowly_at_the_tag() {
        let dir = scratch("clone");
        let cmds: Vec<String> = to_tag(&dir.join("BUI"), "v0.5.1").iter().map(Cmd::display).collect();
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].starts_with("git clone --quiet --depth 1 --branch v0.5.1 https://github.com/ppmpreetham/BUI "), "{}", cmds[0]);
        assert_eq!(generate(&dir).display(), "go generate github.com/ppmpreetham/BUI/bindings/go/bui");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn running_and_building_use_go() {
        assert_eq!(GoApp {}.dev(Path::new("."), &["--x".to_owned()]).unwrap()[0].display(), "go run . --x");
        assert_eq!(GoApp {}.build(Path::new("."), &[]).unwrap()[0].display(), "go build");
        assert!(GoApp {}.tools().contains(&Tool::Git) && GoApp {}.tools().contains(&Tool::Cmake));
    }
}
