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
