use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "scoop-search-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir_all(path.join("buckets/main/bucket")).unwrap();
        Self(path)
    }
    fn package(&self, name: &str) {
        fs::write(
            self.0
                .join("buckets/main/bucket")
                .join(format!("{}.json", name)),
            r#"{"version":"1"}"#,
        )
        .unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_scoop-search"))
            .env("SCOOP", &self.0)
            .env("SCOOP_GLOBAL", self.0.join("global"))
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .args(args)
            .output()
            .unwrap()
    }
    fn text(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn automatic_fallback_and_explicit_fuzzy_have_distinct_output() {
    let fixture = Fixture::new();
    fixture.package("ripgrep");
    fixture.package("ripgrip");
    let exact = fixture.text(&["ripgrep"]);
    assert!(exact.contains("'main' bucket:"));
    assert!(!exact.contains("ripgrip"));
    let fallback = fixture.text(&["ripgrp"]);
    assert!(fallback.contains("Did you mean?"));
    assert!(fallback.contains("main/ripgrep"));
    let explicit = fixture.text(&["--fuzzy", "ripgrep"]);
    assert!(explicit.contains("Fuzzy matches"));
    assert!(explicit.find("main/ripgrep").unwrap() < explicit.find("main/ripgrip").unwrap());
    assert!(fixture.text(&["zzzzzz"]).contains("No matches found."));
    assert!(fixture.text(&["--local", "rg"]).contains("main/ripgrep"));
}

#[test]
fn nucleo_abbreviations_work_in_explicit_and_automatic_modes() {
    let fixture = Fixture::new();
    fixture.package("visual-studio-code");
    fixture.package("ripgrep");
    fixture.package("git");
    for (query, package) in [
        ("vsc", "visual-studio-code"),
        ("RG", "ripgrep"),
        ("gti", "git"),
    ] {
        for flags in [
            vec!["--local", query],
            vec!["--local", "--fuzzy", "--name-only", query],
        ] {
            assert!(fixture.text(&flags).contains(&format!("main/{}", package)));
        }
    }
}

#[test]
fn empty_and_wildcard_queries_still_list_every_package() {
    let fixture = Fixture::new();
    for index in 0..12 {
        fixture.package(&format!("tool{:02}", index));
    }
    for args in [
        &["--local", "--fuzzy"][..],
        &["--local", "--fuzzy", "*"][..],
    ] {
        let text = fixture.text(args);
        assert_eq!(text.matches(" (1)").count(), 12);
        assert!(!text.contains("Fuzzy matches"));
    }
}

#[test]
fn installed_markers_appear_in_literal_and_fuzzy_results() {
    let fixture = Fixture::new();
    fixture.package("ripgrep");
    let current = fixture.0.join("apps/ripgrep/current");
    fs::create_dir_all(&current).unwrap();
    fs::write(current.join("install.json"), r#"{"bucket":"main"}"#).unwrap();
    fs::write(current.join("manifest.json"), r#"{"version":"0.9"}"#).unwrap();
    let global = fixture.0.join("global/apps/ripgrep/current");
    fs::create_dir_all(&global).unwrap();
    fs::write(global.join("install.json"), r#"{"bucket":"main"}"#).unwrap();
    fs::write(global.join("manifest.json"), r#"{"version":"0.8"}"#).unwrap();
    for args in [&["ripgrep"][..], &["ripgrp"][..], &["--fuzzy", "rg"][..]] {
        let text = fixture.text(args);
        assert!(text.contains("ripgrep (1)"));
        assert!(text.contains("[installed: 0.9] [installed globally: 0.8]"));
    }
    fs::write(current.join("install.json"), r#"{"bucket":"extras"}"#).unwrap();
    let text = fixture.text(&["ripgrep"]);
    assert!(!text.contains("[installed: 0.9]"));
    assert!(text.contains("[installed globally: 0.8]"));
}
