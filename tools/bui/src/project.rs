//! A BUI project: what `bui.toml` says about it (which language and options it was made with), found
//! from any folder inside it, or guessed from its files when it has none.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::flavours::{
    Flavour, c::CApp, csharp, csharp::CsharpApp, d, d::DApp, go, go::GoApp, js::JsApp, lua::LuaApp, python::PythonApp, ruby, ruby::RubyApp, rust::RustApp,
};

pub const FILE: &str = "bui.toml";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    React,
    Svelte,
    Vue,
    /// BUI's DOM-like API with no framework.
    Vanilla,
}

impl Framework {
    /// Components in single files (`.svelte`, `.vue`) that BUI's loader compiles, written in plain JavaScript.
    pub const fn has_loader(self) -> bool {
        matches!(self, Self::Svelte | Self::Vue)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[value(name = "ts")]
    TypeScript,
    #[value(name = "js")]
    JavaScript,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Bun,
    Npm,
    Pnpm,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum PyApi {
    /// Blender's own components.
    Components,
    Qt,
    Tk,
}

/// The language a project is in, with that language's options.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "flavour", rename_all = "lowercase")]
pub enum Stack {
    Rust(RustApp),
    Js(JsApp),
    Python(PythonApp),
    Lua(LuaApp),
    C(CApp),
    D(DApp),
    Csharp(CsharpApp),
    Go(GoApp),
    Ruby(RubyApp),
}

impl Stack {
    pub fn app(&self) -> &dyn Flavour {
        match self {
            Self::Rust(a) => a,
            Self::Js(a) => a,
            Self::Python(a) => a,
            Self::Lua(a) => a,
            Self::C(a) => a,
            Self::D(a) => a,
            Self::Csharp(a) => a,
            Self::Go(a) => a,
            Self::Ruby(a) => a,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    #[serde(flatten)]
    pub stack: Stack,
}

#[derive(Serialize, Deserialize)]
struct Doc {
    project: Meta,
}

pub struct Project {
    pub dir: PathBuf,
    pub meta: Meta,
    /// Read from `bui.toml` (not guessed).
    pub declared: bool,
}

impl Meta {
    pub fn to_toml(&self) -> Result<String> {
        let body = toml::to_string_pretty(&Doc { project: self.clone() })?;
        Ok(format!("# Made by `bui new`: bui reads this to know how to run, build and check the project.\n{body}"))
    }
}

/// The project that contains `from`: the nearest folder up with a `bui.toml`, else the folder itself
/// when its files say what it is. A `bui.toml` that cannot be read or parsed is an error, not a reason to
/// look further up (that would run another project's commands).
pub fn find(from: &Path) -> Result<Option<Project>> {
    let start = from.canonicalize().unwrap_or_else(|_| from.to_owned());
    // Windows' canonical paths start with `\\?\`, which many tools cannot read
    let start = start.to_string_lossy().strip_prefix(r"\\?\").map_or_else(|| start.clone(), PathBuf::from);
    for dir in start.ancestors() {
        let path = dir.join(FILE);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let doc: Doc = toml::from_str(&text).with_context(|| format!("{} is not a valid {FILE}: fix it, or remove it", path.display()))?;
        return Ok(Some(Project { dir: dir.to_owned(), meta: doc.project, declared: true }));
    }
    Ok(guess(&start).map(|meta| Project { dir: start, meta, declared: false }))
}

fn read(dir: &Path, file: &str) -> String {
    fs::read_to_string(dir.join(file)).unwrap_or_default()
}

fn guess(dir: &Path) -> Option<Meta> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    let stack = if read(dir, "Cargo.toml").contains("blenderui") {
        Stack::Rust(RustApp {})
    } else if read(dir, "package.json").contains("@blender-ui/") {
        let pkg = read(dir, "package.json");
        let framework = if pkg.contains("@blender-ui/react") {
            Framework::React
        } else if pkg.contains("@blender-ui/svelte") {
            Framework::Svelte
        } else if pkg.contains("@blender-ui/vue") {
            Framework::Vue
        } else {
            Framework::Vanilla
        };
        let pm = if dir.join("bun.lock").exists() || dir.join("bun.lockb").exists() {
            PackageManager::Bun
        } else if dir.join("pnpm-lock.yaml").exists() {
            PackageManager::Pnpm
        } else {
            PackageManager::Npm
        };
        let language = if dir.join("tsconfig.json").exists() { Language::TypeScript } else { Language::JavaScript };
        Stack::Js(JsApp { framework, language, pm })
    } else if depends_on_blenderui(&read(dir, "pyproject.toml")) {
        Stack::Python(PythonApp { api: PyApi::Components })
    } else if read(dir, "main.lua").contains("require(\"bui\")") || read(dir, "main.lua").contains("require('bui')") {
        Stack::Lua(LuaApp {})
    } else if read(dir, "CMakeLists.txt").contains("BUI") && dir.join("src").is_dir() {
        Stack::C(CApp {})
    } else if go::detect(dir) {
        Stack::Go(GoApp {})
    } else if ruby::detect(dir) {
        Stack::Ruby(RubyApp {})
    } else if csharp::detect(dir) {
        Stack::Csharp(CsharpApp {})
    } else if d::detect(dir) {
        Stack::D(DApp {})
    } else {
        return None;
    };
    Some(Meta { name, stack })
}

/// A `pyproject.toml` that depends on blenderui: a requirement of `[project]`'s dependencies or
/// optional ones or of `[dependency-groups]` (uv's), or a key of Poetry's dependencies. The word
/// anywhere else (the project's own name, a description, a comment) is not a dependency.
pub fn depends_on_blenderui(pyproject: &str) -> bool {
    let Ok(doc) = toml::from_str::<toml::Table>(pyproject) else { return false };
    // PEP 508: a requirement starts with the name; blenderui has no separator for PEP 503 to normalize
    let is_bui = |name: &str| name.eq_ignore_ascii_case("blenderui");
    let requirement = |v: &toml::Value| {
        v.as_str().is_some_and(|r| is_bui(r.trim_start().split(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c))).next().unwrap_or("")))
    };
    let list = |v: Option<&toml::Value>| v.and_then(toml::Value::as_array).is_some_and(|a| a.iter().any(requirement));
    let lists = |v: Option<&toml::Value>| v.and_then(toml::Value::as_table).is_some_and(|t| t.values().any(|l| list(Some(l))));
    let project = doc.get("project");
    let poetry = doc.get("tool").and_then(|t| t.get("poetry")).and_then(|p| p.get("dependencies"));
    list(project.and_then(|p| p.get("dependencies")))
        || lists(project.and_then(|p| p.get("optional-dependencies")))
        || lists(doc.get("dependency-groups"))
        || poetry.and_then(toml::Value::as_table).is_some_and(|t| t.keys().any(|k| is_bui(k)))
}

pub fn require(from: &Path) -> Result<Project> {
    find(from)?.with_context(|| "this folder is not a BUI project (no bui.toml). Make one with `bui new` or `bui init`.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flavours::{Vars, c::CApp};

    pub fn every_stack() -> Vec<Stack> {
        let mut stacks = vec![
            Stack::Rust(RustApp {}),
            Stack::Lua(LuaApp {}),
            Stack::C(CApp {}),
            Stack::D(DApp {}),
            Stack::Csharp(CsharpApp {}),
            Stack::Go(GoApp {}),
            Stack::Ruby(RubyApp {}),
        ];
        for api in [PyApi::Components, PyApi::Qt, PyApi::Tk] {
            stacks.push(Stack::Python(PythonApp { api }));
        }
        for pm in [PackageManager::Bun, PackageManager::Npm, PackageManager::Pnpm] {
            for (framework, language) in [
                (Framework::React, Language::TypeScript),
                (Framework::React, Language::JavaScript),
                (Framework::Svelte, Language::JavaScript),
                (Framework::Vue, Language::JavaScript),
                (Framework::Vanilla, Language::TypeScript),
                (Framework::Vanilla, Language::JavaScript),
            ] {
                stacks.push(Stack::Js(JsApp { framework, language, pm }));
            }
        }
        stacks
    }

    #[test]
    fn bui_toml_round_trips_for_every_stack() {
        for stack in every_stack() {
            let meta = Meta { name: "demo".into(), stack };
            let text = meta.to_toml().unwrap();
            let back: Doc = toml::from_str(&text).unwrap();
            assert_eq!(back.project.stack.app().label(), meta.stack.app().label(), "{text}");
        }
    }

    #[test]
    fn every_stack_renders_files_with_no_placeholder_left() {
        let vars = Vars::with_tag("my-app", "v1.2.3");
        for stack in every_stack() {
            let files = stack.app().files(&vars);
            assert!(!files.is_empty(), "{}", stack.app().label());
            assert!(files.iter().any(|f| f.path == ".gitignore"), "{} has no .gitignore", stack.app().label());
            for f in &files {
                // (Vue's own `{{ count }}` has spaces: only the placeholders `bui` fills are looked for)
                for placeholder in ["{{name}}", "{{ident}}", "{{title}}", "{{tag}}"] {
                    assert!(!f.body.contains(placeholder), "{}: {} has {placeholder} left", stack.app().label(), f.path);
                }
                assert!(!f.path.contains("/_") && !f.path.starts_with('_'), "{}: {} keeps its underscore", stack.app().label(), f.path);
            }
        }
    }

    /// What `guess` makes of a folder holding `files`.
    fn guessed(tag: &str, files: &[(&str, &str)]) -> Option<Stack> {
        let dir = std::env::temp_dir().join(format!("bui-guess-{tag}-{}", std::process::id())).join("my-app");
        fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            fs::write(dir.join(name), body).unwrap();
        }
        let stack = guess(&dir).map(|m| m.stack);
        fs::remove_dir_all(dir.parent().unwrap()).unwrap();
        stack
    }

    #[test]
    fn a_folder_without_bui_toml_is_recognized_by_its_files() {
        assert!(matches!(
            guessed("vue", &[("package.json", r#"{"dependencies":{"@blender-ui/native":"1","@blender-ui/vue":"1","vue":"3"}}"#)]),
            Some(Stack::Js(JsApp { framework: Framework::Vue, .. }))
        ));
        assert!(matches!(guessed("go", &[("go.mod", "module x\nrequire github.com/ppmpreetham/BUI/bindings/go v0.0.0\n")]), Some(Stack::Go(_))));
        assert!(guessed("go-other", &[("go.mod", "module x\n")]).is_none());
        assert!(matches!(guessed("ruby", &[("Gemfile", "source \"https://rubygems.org\"\ngem \"blenderui\", \"~> 0.5\"\n")]), Some(Stack::Ruby(_))));
        assert!(guessed("ruby-other", &[("Gemfile", "gem \"rake\"\n")]).is_none());
        assert!(matches!(
            guessed("cs", &[("my-app.csproj", r#"<Project><ItemGroup><PackageReference Include="BlenderUI" Version="0.5.1" /></ItemGroup></Project>"#)]),
            Some(Stack::Csharp(_))
        ));
        assert!(guessed("cs-other", &[("my-app.csproj", "<Project />")]).is_none());
        assert!(matches!(guessed("d", &[("dub.json", r#"{"dependencies":{"blenderui":"~>0.5.1"}}"#)]), Some(Stack::D(_))));
        assert!(guessed("d-other", &[("dub.json", r#"{"dependencies":{"vibe-d":"*"}}"#)]).is_none());
    }

    #[test]
    fn blenderuis_own_pyproject_is_not_an_app() {
        assert!(!depends_on_blenderui("[project]
name = \"blenderui\"
"));
        assert!(depends_on_blenderui("[project]
name = \"x\"
dependencies = [\"blenderui>=0.5\"]
"));
    }

    #[test]
    fn only_a_dependency_entry_counts_as_blenderui() {
        for yes in [
            "[project]
dependencies = [\"BlenderUI[gpu] >=0.5\"]",
            "[project.optional-dependencies]
ui = [\"blenderui\"]",
            "[dependency-groups]
dev = [\"blenderui==0.5.1\"]",
            "[tool.poetry.dependencies]
blenderui = \"^0.5\"",
        ] {
            assert!(depends_on_blenderui(yes), "{yes}");
        }
        for no in [
            "[project]
name = \"blenderui-demo\"
dependencies = [\"numpy\"]",
            "[project]
description = \"made without blenderui\" # blenderui
dependencies = []",
            "[project]
dependencies = [\"blenderui-extras\"]",
            "not toml blenderui",
        ] {
            assert!(!depends_on_blenderui(no), "{no}");
        }
    }

    #[test]
    fn a_project_is_found_from_a_folder_inside_it() {
        let dir = std::env::temp_dir().join(format!("bui-test-{}", std::process::id()));
        let inner = dir.join("src/deep");
        fs::create_dir_all(&inner).unwrap();
        let meta = Meta { name: "demo".into(), stack: Stack::Lua(LuaApp {}) };
        fs::write(dir.join(FILE), meta.to_toml().unwrap()).unwrap();
        let found = find(&inner).unwrap().expect("found");
        assert!(found.declared);
        assert_eq!(found.meta.name, "demo");
        // a broken bui.toml below it is reported, not skipped for the one above
        fs::write(inner.join(FILE), "[project
name = ").unwrap();
        let err = format!("{:#}", find(&inner).err().expect("an error"));
        assert!(err.contains("is not a valid bui.toml"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
