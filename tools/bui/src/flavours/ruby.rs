//! Ruby: the `blenderui` gem (it builds BUI's C kit while it installs) through Bundler, into the project's
//! `vendor/bundle` (nothing global).

use std::path::Path;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::{File, Flavour, Step, Vars, read, render, step};
use crate::check::{self, Check, Fix};
use crate::registry;
use crate::sys::{Cmd, OS, Os};
use crate::tools::{self, Tool};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RubyApp {}

const GEM: &str = "blenderui";

fn bundle(dir: &Path, args: &[&str]) -> Cmd {
    Cmd::new("bundle", args.iter().copied()).in_dir(dir)
}

/// Gems go into the project (`.bundle/config`), so installing needs no administrator rights.
fn local(dir: &Path) -> Cmd {
    bundle(dir, &["config", "set", "--local", "path", "vendor/bundle"])
}

/// `bundle add blenderui`: adds the newest as `~> X.Y` and installs it (which builds the kit).
fn add(dir: &Path) -> Cmd {
    bundle(dir, &["add", GEM])
}

/// Whether a Gemfile has a `gem "blenderui"` line.
fn gemfile_has(gemfile: &str) -> bool {
    gemfile.lines().any(|l| {
        let l = l.trim_start();
        l.strip_prefix("gem").and_then(|r| r.trim_start().strip_prefix(['"', '\''])).is_some_and(|r| r.starts_with(GEM) && r[GEM.len()..].starts_with(['"', '\'']))
    })
}

/// A project in Ruby that uses the gem.
pub fn detect(dir: &Path) -> bool {
    read(dir, "Gemfile").is_some_and(|g| gemfile_has(&g))
}

/// The version of the gem in `Gemfile.lock` (a line `    blenderui (0.5.1)`).
fn locked(lock: &str) -> Option<String> {
    lock.lines().find_map(|l| l.trim_start().strip_prefix(GEM)?.strip_prefix(" (")?.split([')', ' ']).next().map(str::to_owned))
}

impl Flavour for RubyApp {
    fn label(&self) -> String {
        "Ruby".into()
    }

    fn tools(&self) -> Vec<Tool> {
        // the gem builds BUI's kit with CMake and a C++20 compiler (on Windows RubyInstaller's devkit has them)
        let mut tools = vec![Tool::Ruby, Tool::Bundler, Tool::Cmake];
        if OS != Os::Windows {
            tools.push(Tool::Cc);
        }
        tools.extend(tools::x11_headers());
        tools
    }

    fn files(&self, vars: &Vars) -> Vec<File> {
        render(&["ruby"], vars)
    }

    fn deps(&self, dir: &Path) -> Vec<Step> {
        let mut steps = vec![step("Installing gems into this project", local(dir))];
        if detect(dir) {
            steps.push(step("Installing the gems (builds BUI's C kit, a few minutes once)", bundle(dir, &["install"])));
        } else {
            steps.push(step("Adding blenderui (newest; builds BUI's C kit, a few minutes once)", add(dir)));
        }
        steps
    }

    fn dev(&self, dir: &Path, args: &[String]) -> Result<Vec<Cmd>> {
        Ok(vec![bundle(dir, &["exec", "ruby", "app.rb"]).with_args(args)])
    }

    fn build(&self, _dir: &Path, _args: &[String]) -> Result<Vec<Cmd>> {
        bail!("a Ruby app is not built: run it with `bui dev`.")
    }

    fn checks(&self, dir: &Path) -> Vec<Check> {
        let Some(gemfile) = read(dir, "Gemfile") else {
            return vec![Check::fail("Gemfile", "missing", None)];
        };
        if !gemfile_has(&gemfile) {
            return vec![Check::fail("blenderui gem", "not in the Gemfile", Some(Fix::run("adds blenderui", vec![local(dir), add(dir)])))];
        }
        let Some(have) = read(dir, "Gemfile.lock").and_then(|l| locked(&l)) else {
            return vec![Check::fail("blenderui gem", "not installed", Some(Fix::run("installs the gems", vec![local(dir), bundle(dir, &["install"])])))];
        };
        let fix = Fix::run("moves blenderui to the newest", vec![bundle(dir, &["update", GEM])]);
        vec![check::newest("blenderui gem", &have, registry::gem_version(GEM).as_deref(), fix)]
    }

    fn update(&self, dir: &Path) -> Vec<Step> {
        vec![step("Moving blenderui to the newest", bundle(dir, &["update", GEM]))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bui-ruby-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_new_project_has_a_gemfile_and_an_app_that_requires_bui() {
        let files = RubyApp {}.files(&Vars::new("my-app"));
        let body = |path: &str| files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("{path}")).body.clone();
        assert!(body("Gemfile").contains("source \"https://rubygems.org\""));
        assert!(!gemfile_has(&body("Gemfile")), "bundle add puts the gem in");
        assert!(body("app.rb").contains("require \"bui\"") && body("app.rb").contains("\"My App\""));
        assert!(body(".gitignore").contains("vendor/bundle"));
    }

    #[test]
    fn the_gem_is_found_in_the_gemfile_in_its_spellings() {
        for yes in ["gem \"blenderui\"", "  gem 'blenderui', '~> 0.5'", "gem  \"blenderui\", \"~> 0.5\""] {
            assert!(gemfile_has(yes), "{yes}");
        }
        for no in ["gem \"blenderui-extras\"", "# gem \"blenderui\"", "gem \"rake\"", "source \"https://rubygems.org\"\n# blenderui"] {
            assert!(!gemfile_has(no), "{no}");
        }
    }

    #[test]
    fn the_locked_version_is_read() {
        let lock = "GEM\n  remote: https://rubygems.org/\n  specs:\n    blenderui (0.5.1)\n    rake (13.2.1)\n\nDEPENDENCIES\n  blenderui (~> 0.5)\n";
        assert_eq!(locked(lock).as_deref(), Some("0.5.1"));
        assert_eq!(locked("    blenderui-extras (1.0.0)\n"), None);
        assert_eq!(locked(""), None);
    }

    #[test]
    fn the_steps_install_into_the_project_and_add_the_gem_only_where_missing() {
        let dir = scratch("deps");
        let lines = |steps: Vec<Step>| steps.iter().map(|s| s.work.display()).collect::<Vec<_>>();
        std::fs::write(dir.join("Gemfile"), "source \"https://rubygems.org\"\n").unwrap();
        assert_eq!(lines(RubyApp {}.deps(&dir)), ["bundle config set --local path vendor/bundle", "bundle add blenderui"]);
        std::fs::write(dir.join("Gemfile"), "source \"https://rubygems.org\"\ngem \"blenderui\"\n").unwrap();
        assert_eq!(lines(RubyApp {}.deps(&dir)), ["bundle config set --local path vendor/bundle", "bundle install"]);
        assert!(detect(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn running_uses_bundler_and_there_is_no_build() {
        let cmd = &RubyApp {}.dev(Path::new("."), &[]).unwrap()[0];
        assert_eq!(cmd.display(), "bundle exec ruby app.rb");
        assert!(RubyApp {}.build(Path::new("."), &[]).is_err());
        assert_eq!(RubyApp {}.update(Path::new("."))[0].work.display(), "bundle update blenderui");
    }

    #[test]
    fn a_project_without_the_gem_is_fixed_by_adding_it() {
        let dir = scratch("nogem");
        std::fs::write(dir.join("Gemfile"), "source \"https://rubygems.org\"\n").unwrap();
        let checks = RubyApp {}.checks(&dir);
        let cmds: Vec<String> = checks[0].fix().expect("a fix").work.iter().map(crate::check::Work::display).collect();
        assert_eq!(cmds, ["bundle config set --local path vendor/bundle", "bundle add blenderui"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
