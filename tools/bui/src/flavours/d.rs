//! D: the `blenderui` DUB package (the module is `bui`), built with LDC.

use std::path::Path;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Adopted, File, Flavour, Step, Vars, read, read_manifest, render, step, work_step};
use crate::check::{self, Check, Fix, Work};
use crate::registry;
use crate::sys::Cmd;
use crate::tools::Tool;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DApp {}

const PACKAGE: &str = "blenderui";

/// DUB, with LDC as the compiler (the one the package is tested with: DUB would default to DMD).
fn dub(dir: &Path, args: &[&str]) -> Cmd {
    Cmd::new("dub", args.iter().copied()).in_dir(dir).with_env(vec![("DC".to_owned(), "ldc2".to_owned())])
}

/// `dub add blenderui`: adds the newest, as `~>X.Y.Z`.
fn add(dir: &Path) -> Cmd {
    dub(dir, &["add", PACKAGE])
}

/// Whether a `dub.json` lists the package among its dependencies.
fn json_depends(manifest: &Value) -> bool {
    manifest["dependencies"].get(PACKAGE).is_some()
}

/// A project in D that uses the package: its `dub.json` (or `dub.sdl`) depends on it.
pub fn detect(dir: &Path) -> bool {
    let json = read(dir, "dub.json").and_then(|t| serde_json::from_str::<Value>(&t).ok());
    json.as_ref().is_some_and(json_depends) || read(dir, "dub.sdl").is_some_and(|t| t.lines().any(|l| l.trim_start().starts_with(&format!("dependency \"{PACKAGE}\""))))
}

/// `text` (a `dub.json`) with the requirement of the package moved to `~>newest`: `None` when there is none to move
/// (no such dependency, a path or a repository instead of a version) or it already is that.
fn with_requirement(text: &str, newest: &str) -> Option<String> {
    let mut doc: Value = serde_json::from_str(text).ok()?;
    let want = json!(format!("~>{newest}"));
    let slot = match doc.get_mut("dependencies")?.get_mut(PACKAGE)? {
        Value::String(_) => doc.get_mut("dependencies")?.get_mut(PACKAGE)?,
        Value::Object(o) if o.get("version").is_some_and(Value::is_string) => o.get_mut("version")?,
        _ => return None,
    };
    if *slot == want {
        return None;
    }
    *slot = want;
    Some(format!("{}\n", serde_json::to_string_pretty(&doc).ok()?))
}

/// The edit that moves the requirement in `dub.json`: the whole file, as it was read.
fn move_requirement(dir: &Path, newest: &str) -> Option<Work> {
    let was = read(dir, "dub.json")?;
    let to = with_requirement(&was, newest)?;
    Some(Work::Rewrite { path: dir.join("dub.json"), at: 0..was.len(), was, to })
}

/// The version DUB resolved, from `dub.selections.json`.
fn selected(dir: &Path) -> Option<String> {
    let doc: Value = serde_json::from_str(&read(dir, "dub.selections.json")?).ok()?;
    let v = &doc["versions"][PACKAGE];
    v.as_str().or_else(|| v["version"].as_str()).map(str::to_owned)
}

impl Flavour for DApp {
    fn label(&self) -> String {
        "D".into()
    }

    fn tools(&self) -> Vec<Tool> {
        // the package builds BUI's C kit with CMake on the first build
        vec![Tool::Ldc, Tool::Dub, Tool::Cmake, Tool::Cc]
    }

    /// An existing `dub.json` is kept (`dub add` puts the package in it); a `dub.sdl` is not rewritten as JSON.
    fn adopt(&self, dir: &Path, _name: &str) -> Result<Vec<Adopted>> {
        let Some(text) = read_manifest(dir, "dub.json")? else {
            if dir.join("dub.sdl").is_file() {
                bail!("this folder has a dub.sdl, and bui works with a dub.json (DUB would not know which to read): turn it into one with `dub convert -f json`, then run `bui init` again");
            }
            return Ok(Vec::new());
        };
        let doc = serde_json::from_str::<Value>(&text).ok().filter(Value::is_object);
        let Some(doc) = doc else {
            bail!("{} is not a JSON object: fix it first, then run `bui init` again", dir.join("dub.json").display());
        };
        let note = if json_depends(&doc) {
            "already depends on blenderui; left as it was"
        } else {
            "kept as it was; blenderui is added to its dependencies"
        };
        Ok(vec![Adopted { path: "dub.json", note: note.to_owned() }])
    }

    fn files(&self, vars: &Vars) -> Vec<File> {
        render(&["d"], vars)
    }

    fn deps(&self, dir: &Path) -> Vec<Step> {
        let mut steps = Vec::new();
        if !detect(dir) {
            steps.push(step("Adding blenderui (newest)", add(dir)));
        }
        steps.push(step("Compiling BUI once (a few minutes now, seconds after)", dub(dir, &["build"])));
        steps
    }

    fn dev(&self, dir: &Path, args: &[String]) -> Result<Vec<Cmd>> {
        Ok(vec![dub(dir, &["run"]).with_args(args)])
    }

    fn build(&self, dir: &Path, args: &[String]) -> Result<Vec<Cmd>> {
        Ok(vec![dub(dir, &["build", "-b", "release"]).with_args(args)])
    }

    fn checks(&self, dir: &Path) -> Vec<Check> {
        if read(dir, "dub.json").is_none() && read(dir, "dub.sdl").is_none() {
            return vec![Check::fail("dub.json", "missing", None)];
        }
        if !detect(dir) {
            return vec![Check::fail("blenderui package", "not a dependency", Some(Fix::run("adds blenderui", vec![add(dir)])))];
        }
        let Some(have) = selected(dir) else {
            return vec![Check::fail("blenderui package", "not fetched yet", Some(Fix::run("fetches blenderui", vec![dub(dir, &["upgrade"])])))];
        };
        let newest = registry::dub_version(PACKAGE);
        let work: Vec<Work> = newest
            .iter()
            .flat_map(|v| move_requirement(dir, v).into_iter().chain([Work::Run(dub(dir, &["upgrade"]))]))
            .collect();
        vec![check::newest("blenderui package", &have, newest.as_deref(), Fix::edit("moves blenderui to the newest".into(), work))]
    }

    fn update(&self, dir: &Path) -> Vec<Step> {
        let Some(newest) = registry::dub_version(PACKAGE) else {
            return Vec::new();
        };
        let mut steps: Vec<Step> = move_requirement(dir, &newest).map(|w| work_step("Moving blenderui to the newest", w)).into_iter().collect();
        steps.push(step("Fetching it", dub(dir, &["upgrade"])));
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bui-d-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_new_project_is_a_dub_package_with_the_app_in_source() {
        let files = DApp {}.files(&Vars::new("my-app"));
        let body = |path: &str| files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("{path}")).body.clone();
        let manifest: Value = serde_json::from_str(&body("dub.json")).unwrap();
        assert_eq!(manifest["name"], "my-app");
        assert_eq!(manifest["targetType"], "executable");
        assert!(body("source/app.d").contains("import bui;") && body("source/app.d").contains("\"My App\""));
        assert!(body(".gitignore").contains("/my-app"));
    }

    #[test]
    fn the_compiler_is_ldc_and_arguments_follow() {
        let cmd = &DApp {}.dev(Path::new("."), &["--flag".to_owned()]).unwrap()[0];
        assert_eq!(cmd.display(), "dub run --flag");
        assert_eq!(cmd.env, [("DC".to_owned(), "ldc2".to_owned())]);
        assert_eq!(DApp {}.build(Path::new("."), &[]).unwrap()[0].display(), "dub build -b release");
    }

    #[test]
    fn a_project_is_recognized_by_its_dependency() {
        let dir = scratch("detect");
        assert!(!detect(&dir));
        std::fs::write(dir.join("dub.json"), r#"{"name":"bui","dependencies":{}}"#).unwrap();
        assert!(!detect(&dir), "BUI's own package is not an app");
        std::fs::write(dir.join("dub.json"), r#"{"name":"x","dependencies":{"blenderui":"~>0.5.1"}}"#).unwrap();
        assert!(detect(&dir));
        std::fs::remove_file(dir.join("dub.json")).unwrap();
        std::fs::write(dir.join("dub.sdl"), "name \"x\"\ndependency \"blenderui\" version=\"~>0.5.1\"\n").unwrap();
        assert!(detect(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_requirement_moves_and_only_that() {
        let text = r#"{"name":"x","dependencies":{"vibe-d":"~>0.9.0","blenderui":"~>0.5.1"}}"#;
        let moved: Value = serde_json::from_str(&with_requirement(text, "0.6.0").unwrap()).unwrap();
        assert_eq!(moved["dependencies"]["blenderui"], "~>0.6.0");
        assert_eq!(moved["dependencies"]["vibe-d"], "~>0.9.0");
        let keys: Vec<&String> = moved["dependencies"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["vibe-d", "blenderui"], "their order is kept");
        let object = r#"{"dependencies":{"blenderui":{"version":"~>0.5.1"}}}"#;
        assert!(with_requirement(object, "0.6.0").unwrap().contains("~>0.6.0"));
        assert!(with_requirement(text, "0.5.1").is_none(), "already there");
        assert!(with_requirement(r#"{"dependencies":{"blenderui":{"path":"../b"}}}"#, "0.6.0").is_none(), "a path stays");
        assert!(with_requirement(r#"{"dependencies":{}}"#, "0.6.0").is_none());
        assert!(with_requirement("not json", "0.6.0").is_none());
    }

    #[test]
    fn moving_the_requirement_rewrites_the_file_it_read() {
        let dir = scratch("move");
        std::fs::write(dir.join("dub.json"), r#"{"dependencies":{"blenderui":"~>0.5.1"}}"#).unwrap();
        let work = move_requirement(&dir, "0.6.0").unwrap();
        assert!(work.perform("test").unwrap());
        assert!(std::fs::read_to_string(dir.join("dub.json")).unwrap().contains("~>0.6.0"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn init_keeps_a_dub_json_and_stops_at_a_dub_sdl() {
        let dir = scratch("adopt");
        assert!(DApp {}.adopt(&dir, "demo").unwrap().is_empty(), "nothing there, nothing adopted");
        std::fs::write(dir.join("dub.sdl"), "name \"x\"\n").unwrap();
        assert!(DApp {}.adopt(&dir, "demo").is_err(), "a dub.sdl is not turned into JSON");
        std::fs::write(dir.join("dub.json"), r#"{"name":"mine","dependencies":{"vibe-d":"*"}}"#).unwrap();
        let adopted = DApp {}.adopt(&dir, "demo").unwrap();
        assert!(adopted[0].note.contains("is added"));
        assert!(std::fs::read_to_string(dir.join("dub.json")).unwrap().contains("vibe-d"), "not touched");
        // the steps add the package only where it is not already
        assert!(DApp {}.deps(&dir).iter().any(|s| s.work.display().starts_with("dub add blenderui")));
        std::fs::write(dir.join("dub.json"), r#"{"dependencies":{"blenderui":"~>0.5.1"}}"#).unwrap();
        assert!(!DApp {}.deps(&dir).iter().any(|s| s.work.display().starts_with("dub add")));
        std::fs::write(dir.join("dub.json"), "{ nope").unwrap();
        assert!(DApp {}.adopt(&dir, "demo").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_resolved_version_comes_from_the_selections() {
        let dir = scratch("selected");
        std::fs::write(dir.join("dub.selections.json"), r#"{"fileVersion":1,"versions":{"blenderui":"0.5.1"}}"#).unwrap();
        assert_eq!(selected(&dir).as_deref(), Some("0.5.1"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_project_without_the_dependency_is_fixed_by_adding_it() {
        let dir = scratch("nodep");
        std::fs::write(dir.join("dub.json"), r#"{"name":"x","dependencies":{}}"#).unwrap();
        let checks = DApp {}.checks(&dir);
        let cmds: Vec<String> = checks[0].fix().expect("a fix").work.iter().map(Work::display).collect();
        assert_eq!(cmds, ["dub add blenderui"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
