use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "scoop-search-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, content: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fn scoop(&self) -> Scoop {
        Scoop {
            dir: self.0.clone(),
            buckets_dir: self.0.join("buckets"),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn arguments_keep_legacy_forms_and_allow_offline_name_search() {
    let parse = |args: &[&str]| parse_args(args.iter().map(|s| s.to_string()));
    assert!(!parse(&["search", "FOO"]).unwrap().exclude_bin);
    assert_eq!(parse(&["search", "--bin", "FOO"]).unwrap().query, "foo");
    assert_eq!(parse(&["search", "*"]).unwrap().query, "");
    assert_eq!(parse(&["search"]).unwrap().query, "");
    let args = parse(&["search", "foo", "--local", "--name-only"]).unwrap();
    assert!(args.local_only && args.exclude_bin);
    assert!(parse(&["search", "foo"]).unwrap().local_only);
    assert!(parse(&["search", "--fuzzy", "foo"]).unwrap().local_only);
    assert!(!parse(&["search", "--remote", "foo"]).unwrap().local_only);
    assert!(
        !parse(&["search", "--fuzzy", "foo", "--remote"])
            .unwrap()
            .local_only
    );
    assert!(
        parse(&["search", "--remote", "--local", "foo"])
            .unwrap()
            .local_only
    );
    assert!(
        !parse(&["search", "--local", "--remote", "foo"])
            .unwrap()
            .local_only
    );
    assert!(parse(&["search", "--", "--remote"]).unwrap().local_only);
    assert!(parse(&["search", "--fuzzy", "gti"]).unwrap().fuzzy);
    assert!(!parse(&["search", "--", "--fuzzy"]).unwrap().fuzzy);
    assert_eq!(
        parse(&["search", "--", "--literal"]).unwrap().query,
        "--literal"
    );
    assert!(parse(&["search", "--unknown"]).is_err());
    assert!(parse(&["search", "a", "b"]).is_err());
}

#[test]
fn search_combines_names_and_bins_across_bucket_layouts() {
    let f = Fixture::new();
    fs::create_dir_all(f.0.join("buckets/empty/bucket")).unwrap();
    f.write("buckets/main/bucket/foo.json", r#"{"version":"1"}"#);
    f.write(
        "buckets/main/bucket/nested/tool.json",
        r#"{"version":"2","bin":[["bin/foo.exe","foo","--secret"],"bin/foo.exe"]}"#,
    );
    f.write(
        "buckets/legacy/another.json",
        r#"{"version":"3","bin":"foo.cmd"}"#,
    );
    f.write(
        "buckets/legacy/.git/ignored.json",
        r#"{"version":"0","bin":"foo"}"#,
    );
    f.write("buckets/main/bucket/invalid.json", "invalid");
    f.write("buckets/main/bucket/foo.txt", r#"{"version":"0"}"#);
    f.write("buckets/not-a-directory", "ignored");
    let paths = Bucket::paths(&f.scoop()).unwrap();
    assert_eq!(paths.len(), 3);
    let results = Bucket::search(&paths, "foo", false).unwrap();
    assert_eq!(
        results.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
        ["legacy", "main"]
    );
    assert_eq!(
        results[1]
            .apps
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["foo", "tool"]
    );
    assert_eq!(results[1].apps[1].bin, ["foo", "foo.exe"]);
    let names = Bucket::search(&paths, "foo", true).unwrap();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].apps.len(), 1);
    assert!(Bucket::search(&paths, "secret", false).unwrap().is_empty());
}

#[test]
fn xdg_config_resolves_without_userprofile() {
    let f = Fixture::new();
    f.write("scoop/config.json", r#"{"root_path":"D:\\My Scoop"}"#);
    let scoop = Scoop::resolve(None, None, Some(f.0.clone())).unwrap();
    assert_eq!(scoop.dir, PathBuf::from(r"D:\My Scoop"));
    assert_eq!(
        Scoop::resolve(Some("override".into()), None, Some(f.0.clone()))
            .unwrap()
            .dir,
        PathBuf::from("override")
    );
}

#[test]
fn missing_buckets_report_error_instead_of_panicking() {
    let f = Fixture::new();
    assert!(Bucket::paths(&f.scoop()).is_err());
}

#[test]
fn installed_status_tracks_source_scope_and_version() {
    let f = Fixture::new();
    f.write("apps/git/current/install.json", r#"{"bucket":"main"}"#);
    f.write("apps/git/current/manifest.json", r#"{"version":"1.0"}"#);
    f.write(
        "global/apps/git/current/install.json",
        r#"{"bucket":"extras"}"#,
    );
    f.write(
        "global/apps/git/current/manifest.json",
        r#"{"version":"2.0"}"#,
    );
    f.write("apps/tool/current/install.json", "{}");
    f.write("apps/tool/current/manifest.json", r#"{"version":"3.0"}"#);
    let installed = installed::Installed::from_roots(&f.0, Some(&f.0.join("global")), false);
    assert_eq!(installed.label("MAIN", "GIT"), " [installed: 1.0]");
    assert_eq!(
        installed.label("extras", "git"),
        " [installed globally: 2.0]"
    );
    assert_eq!(installed.label("other", "git"), "");
    assert_eq!(
        installed.label("main", "tool"),
        " [installed (source unknown): 3.0]"
    );
    assert_eq!(installed.label("main", "absent"), "");
}

#[test]
fn installed_status_handles_no_junction_and_incomplete_installs() {
    let f = Fixture::new();
    f.write("apps/git/current/install.json", r#"{"bucket":"wrong"}"#);
    f.write("apps/git/current/manifest.json", r#"{"version":"0"}"#);
    f.write("apps/git/1.0/install.json", r#"{"bucket":"main"}"#);
    f.write("apps/git/1.0/manifest.json", r#"{"version":"1.0"}"#);
    f.write("apps/git/_2.0.old/install.json", r#"{"bucket":"wrong"}"#);
    f.write("apps/git/_2.0.old/manifest.json", r#"{"version":"2.0"}"#);
    f.write(
        "apps/incomplete/current/manifest.json",
        r#"{"version":"1.0"}"#,
    );
    f.write("apps/broken/current/install.json", "invalid");
    f.write("apps/broken/current/manifest.json", r#"{"version":"1.0"}"#);
    let installed = installed::Installed::from_roots(&f.0, None, true);
    assert_eq!(installed.label("main", "git"), " [installed: 1.0]");
    assert_eq!(installed.label("main", "incomplete"), "");
    assert_eq!(installed.label("main", "broken"), "");
    fs::remove_dir_all(f.0.join("apps/git/current")).unwrap();
    let installed = installed::Installed::from_roots(&f.0, None, false);
    assert_eq!(installed.label("main", "git"), " [installed: 1.0]");
}

#[test]
fn fuzzy_search_ranks_names_aliases_and_respects_name_only() {
    let f = Fixture::new();
    f.write("buckets/main/bucket/git.json", r#"{"version":"1"}"#);
    f.write("buckets/main/bucket/git-lfs.json", r#"{"version":"1"}"#);
    f.write(
        "buckets/extra/bucket/tool.json",
        r#"{"version":"2","bin":[["dir/tool.exe","ripgrep","--firefox"],"ripgrep.exe"]}"#,
    );
    let paths = Bucket::paths(&f.scoop()).unwrap();
    let typo = Bucket::fuzzy_search(&paths, "gti", false).unwrap();
    assert_eq!(typo[0].app.name, "git");
    let ranked = Bucket::fuzzy_search(&paths, "git", true).unwrap();
    assert_eq!(ranked[0].app.name, "git");
    assert_eq!(ranked[1].app.name, "git-lfs");
    let alias = Bucket::fuzzy_search(&paths, "ripgrp", false).unwrap();
    assert_eq!(alias.len(), 1);
    assert_eq!(alias[0].app.name, "tool");
    assert_eq!(alias[0].app.bin[0], "ripgrep");
    let abbreviation = Bucket::fuzzy_search(&paths, "rg", false).unwrap();
    assert_eq!(abbreviation.len(), 1);
    assert_eq!(abbreviation[0].app.name, "tool");
    assert!(Bucket::fuzzy_search(&paths, "rg", true).unwrap().is_empty());
    assert!(Bucket::fuzzy_search(&paths, "ripgrp", true)
        .unwrap()
        .is_empty());
    assert!(Bucket::fuzzy_search(&paths, "firefpx", false)
        .unwrap()
        .is_empty());
}

#[test]
fn fuzzy_results_have_global_limit_and_stable_ties() {
    let f = Fixture::new();
    for index in 0..15 {
        f.write(
            &format!("buckets/main/bucket/tool{:02}.json", index),
            r#"{"version":"1"}"#,
        );
    }
    let paths = Bucket::paths(&f.scoop()).unwrap();
    let results = Bucket::fuzzy_search(&paths, "tool", false).unwrap();
    assert_eq!(results.len(), 10);
    assert_eq!(results[0].app.name, "tool00");
    assert_eq!(results[9].app.name, "tool09");
}

#[test]
fn remote_catalog_yields_literal_and_fuzzy_results_without_refetching() {
    let apps = vec![App {
        name: "ripgrep".into(),
        version: String::new(),
        bin: Vec::new(),
    }];
    let (literal, similar) = Bucket::remote_results(
        "extras",
        apps.clone(),
        "ripgrp",
        &fuzzy::Query::new("ripgrp"),
    );
    assert!(literal.apps.is_empty());
    assert_eq!(similar.len(), 1);
    assert!(similar[0].remote);
    let (literal, abbreviated) =
        Bucket::remote_results("extras", apps.clone(), "rg", &fuzzy::Query::new("rg"));
    assert!(literal.apps.is_empty());
    assert_eq!(abbreviated[0].app.name, "ripgrep");
    let (literal, _) =
        Bucket::remote_results("extras", apps, "ripgrep", &fuzzy::Query::new("ripgrep"));
    assert_eq!(literal.apps.len(), 1);
}
