//! C#: the `BlenderUI` NuGet package (it carries the kit for the common platforms) on the .NET 8 SDK.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::{Adopted, File, Flavour, Step, Vars, render, step};
use crate::check::{self, Check, Fix};
use crate::registry;
use crate::sys::Cmd;
use crate::tools::Tool;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CsharpApp {}

const PACKAGE: &str = "BlenderUI";

fn dotnet(dir: &Path, args: &[&str]) -> Cmd {
    Cmd::new("dotnet", args.iter().copied()).in_dir(dir)
}

/// `dotnet add package BlenderUI`: adds the newest release, or moves a reference to it.
fn add(dir: &Path) -> Cmd {
    dotnet(dir, &["add", "package", PACKAGE])
}

/// The project files (`*.csproj`) directly in `dir`.
fn project_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("csproj"))).collect();
    files.sort();
    files
}

/// What a project says about the package.
#[derive(Debug, PartialEq, Eq)]
enum Reference {
    Missing,
    /// A reference with no version of its own (central package management).
    Unversioned,
    Version(String),
}

/// What the `PackageReference` of BlenderUI in `project` says.
fn reference(project: &str) -> Reference {
    let found = project.split("<PackageReference").skip(1).find_map(|chunk| {
        let tag = chunk.split_once("</PackageReference>").map_or_else(|| chunk.split_once("/>").map_or(chunk, |(t, _)| t), |(t, _)| t);
        (attribute(tag, "Include")?.eq_ignore_ascii_case(PACKAGE)).then(|| attribute(tag, "Version").or_else(|| element(tag, "Version")))
    });
    match found {
        None => Reference::Missing,
        Some(None) => Reference::Unversioned,
        Some(Some(v)) => Reference::Version(v),
    }
}

/// `name="value"` in `tag`.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!("{name}=\""))? + name.len() + 2;
    tag[at..].split_once('"').map(|(v, _)| v.trim().to_owned())
}

/// `<name>value</name>` in `tag`.
fn element(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!("<{name}>"))? + name.len() + 2;
    tag[at..].split_once('<').map(|(v, _)| v.trim().to_owned())
}

/// A project in C# that uses the package.
pub fn detect(dir: &Path) -> bool {
    project_files(dir).iter().any(|p| std::fs::read_to_string(p).is_ok_and(|t| reference(&t) != Reference::Missing))
}

/// The project the commands run on, with the package referenced or not.
fn project(dir: &Path) -> Option<String> {
    let file = project_files(dir).into_iter().next()?;
    std::fs::read_to_string(file).ok()
}

const fn csproj() -> &'static str {
    "<Project Sdk=\"Microsoft.NET.Sdk\">\n\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    <!-- runs on a newer runtime when .NET 8 itself is not installed -->\n    <RollForward>Major</RollForward>\n    <Nullable>enable</Nullable>\n  </PropertyGroup>\n\n</Project>\n"
}

impl Flavour for CsharpApp {
    fn label(&self) -> String {
        "C#".into()
    }

    fn tools(&self) -> Vec<Tool> {
        // the package carries the kit for Windows x64, Linux x64 and macOS: no compiler
        vec![Tool::Dotnet]
    }

    /// `dotnet` works on the one project file of a folder, so another project file stops `init`; the same name is theirs and is kept.
    fn adopt(&self, dir: &Path, name: &str) -> Result<Vec<Adopted>> {
        let ours = format!("{name}.csproj");
        let files = project_files(dir);
        if let Some(other) = files.iter().find(|p| p.file_name().is_some_and(|n| n.to_string_lossy() != ours)) {
            bail!(
                "{} is already here, and `dotnet` works on one project file per folder: add BlenderUI to it yourself with `dotnet add package {PACKAGE}`, or rename it to {ours}, then run `bui init` again",
                other.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        // theirs is kept (`init` says so), and `dotnet add package` puts the package in it
        Ok(Vec::new())
    }

    fn files(&self, vars: &Vars) -> Vec<File> {
        let mut files = render(&["csharp"], vars);
        files.push(File { path: format!("{}.csproj", vars.name), body: csproj().to_owned() });
        files
    }

    fn deps(&self, dir: &Path) -> Vec<Step> {
        let mut steps = Vec::new();
        if !detect(dir) {
            steps.push(step("Adding BlenderUI (newest)", add(dir)));
        }
        steps.push(step("Building once", dotnet(dir, &["build"])));
        steps
    }

    fn dev(&self, dir: &Path, args: &[String]) -> Result<Vec<Cmd>> {
        Ok(vec![dotnet(dir, &["run"]).with_args(args)])
    }

    fn build(&self, dir: &Path, args: &[String]) -> Result<Vec<Cmd>> {
        Ok(vec![dotnet(dir, &["build", "-c", "Release"]).with_args(args)])
    }

    fn checks(&self, dir: &Path) -> Vec<Check> {
        let Some(project) = project(dir) else {
            return vec![Check::fail("project file", "no .csproj in this folder", None)];
        };
        let fix = Fix::run("moves BlenderUI to the newest", vec![add(dir)]);
        match reference(&project) {
            Reference::Missing => vec![Check::fail("BlenderUI package", "not referenced in the project", Some(Fix::run("adds BlenderUI", vec![add(dir)])))],
            Reference::Unversioned => vec![Check::warn("BlenderUI package", "the reference has no version of its own (central package management?)", None)],
            Reference::Version(have) => vec![check::newest("BlenderUI package", &have, registry::nuget_version(PACKAGE).as_deref(), fix)],
        }
    }

    fn update(&self, dir: &Path) -> Vec<Step> {
        vec![step("Moving BlenderUI to the newest", add(dir))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bui-cs-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_new_project_is_a_net8_exe_named_after_the_app() {
        let files = CsharpApp {}.files(&Vars::new("my-app"));
        let body = |path: &str| files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("{path}")).body.clone();
        let project = body("my-app.csproj");
        assert!(project.contains("<TargetFramework>net8.0</TargetFramework>") && project.contains("<OutputType>Exe</OutputType>"));
        assert!(project.contains("<RollForward>Major</RollForward>"));
        assert!(body("Program.cs").contains("using Bui;") && body("Program.cs").contains("\"My App\""));
        assert!(body(".gitignore").contains("bin/"));
    }

    #[test]
    fn the_package_reference_is_read_in_every_spelling() {
        let version = |v: &str| Reference::Version(v.to_owned());
        assert_eq!(reference(r#"<ItemGroup><PackageReference Include="BlenderUI" Version="0.5.1" /></ItemGroup>"#), version("0.5.1"));
        assert_eq!(reference(r#"<PackageReference Version="0.5.1" Include="blenderui"/>"#), version("0.5.1"));
        assert_eq!(reference("<PackageReference Include=\"BlenderUI\">\n  <Version>0.6.0</Version>\n</PackageReference>"), version("0.6.0"));
        assert_eq!(reference(r#"<PackageReference Include="BlenderUI" />"#), Reference::Unversioned);
        assert_eq!(reference(r#"<PackageReference Include="Newtonsoft.Json" Version="13.0.1" />"#), Reference::Missing);
        assert_eq!(reference(r#"<PackageReference Include="BlenderUI.Extras" Version="1.0.0" />"#), Reference::Missing, "another package");
        assert_eq!(reference("<Project />"), Reference::Missing);
    }

    #[test]
    fn a_project_is_recognized_by_its_reference() {
        let dir = scratch("detect");
        assert!(!detect(&dir));
        std::fs::write(dir.join("a.csproj"), csproj()).unwrap();
        assert!(!detect(&dir), "no reference yet");
        std::fs::write(dir.join("a.csproj"), r#"<Project><ItemGroup><PackageReference Include="BlenderUI" Version="0.5.1" /></ItemGroup></Project>"#).unwrap();
        assert!(detect(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn init_keeps_a_project_of_the_same_name_and_stops_at_another() {
        let dir = scratch("adopt");
        assert!(CsharpApp {}.adopt(&dir, "demo").unwrap().is_empty());
        std::fs::write(dir.join("demo.csproj"), "<Project />").unwrap();
        assert!(CsharpApp {}.adopt(&dir, "demo").is_ok());
        std::fs::write(dir.join("Other.csproj"), "<Project />").unwrap();
        let err = CsharpApp {}.adopt(&dir, "demo").err().expect("an error").to_string();
        assert!(err.contains("Other.csproj") && err.contains("dotnet add package BlenderUI"), "{err}");
        assert_eq!(std::fs::read_to_string(dir.join("demo.csproj")).unwrap(), "<Project />", "nothing is touched");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_package_is_added_only_where_it_is_missing() {
        let dir = scratch("deps");
        std::fs::write(dir.join("a.csproj"), csproj()).unwrap();
        let line = |steps: Vec<Step>| steps.iter().map(|s| s.work.display()).collect::<Vec<_>>();
        assert_eq!(line(CsharpApp {}.deps(&dir)), ["dotnet add package BlenderUI", "dotnet build"]);
        std::fs::write(dir.join("a.csproj"), r#"<PackageReference Include="BlenderUI" Version="0.5.1" />"#).unwrap();
        assert_eq!(line(CsharpApp {}.deps(&dir)), ["dotnet build"]);
        let checks = CsharpApp {}.checks(&dir);
        assert_eq!(checks.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn running_and_building_use_dotnet() {
        assert_eq!(CsharpApp {}.dev(Path::new("."), &[]).unwrap()[0].display(), "dotnet run");
        assert_eq!(CsharpApp {}.build(Path::new("."), &[]).unwrap()[0].display(), "dotnet build -c Release");
        assert_eq!(CsharpApp {}.update(Path::new("."))[0].work.display(), "dotnet add package BlenderUI");
    }
}
