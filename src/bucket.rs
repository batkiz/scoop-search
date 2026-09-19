use crate::{app::App, scoop::Scoop};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

#[derive(Debug, PartialEq)]
pub struct Bucket {
    pub name: String,
    pub apps: Vec<App>,
}

impl Bucket {
    pub fn paths(scoop: &Scoop) -> io::Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(&scoop.buckets_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn manifests(dir: &Path, paths: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
                Self::manifests(&entry.path(), paths)?;
            } else if kind.is_file() && entry.path().extension().is_some_and(|e| e == "json") {
                paths.push(entry.path());
            }
        }
        Ok(())
    }

    pub fn search(paths: &[PathBuf], query: &str, names_only: bool) -> io::Result<Vec<Self>> {
        let mut jobs = Vec::new();
        let mut buckets = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            buckets.push(Self {
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                apps: Vec::new(),
            });
            let dir = path.join("bucket");
            let dir = if dir.is_dir() { dir } else { path.clone() };
            let mut manifests = Vec::new();
            Self::manifests(&dir, &mut manifests)
                .map_err(|e| io::Error::new(e.kind(), format!("{}: {}", dir.display(), e)))?;
            for path in manifests {
                // Preserve the cheap filename-only mode: don't read unrelated JSON.
                if names_only
                    && !path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(query)
                {
                    continue;
                }
                jobs.push((index, path));
            }
        }
        let next = AtomicUsize::new(0);
        let workers = thread::available_parallelism()
            .map_or(1, |n| n.get())
            .min(16)
            .min(jobs.len());
        let matches = thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut matches = Vec::new();
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some((bucket, path)) = jobs.get(index) else {
                                break;
                            };
                            if let Some(app) =
                                App::read(path).and_then(|a| a.matching(query, names_only))
                            {
                                matches.push((*bucket, app));
                            }
                        }
                        matches
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("search worker panicked"))
                .collect::<Vec<_>>()
        });
        for (index, app) in matches {
            buckets[index].apps.push(app);
        }
        for bucket in &mut buckets {
            bucket.apps.sort_by(|a, b| a.name.cmp(&b.name));
        }
        buckets.retain(|b| !b.apps.is_empty());
        buckets.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(buckets)
    }

    pub fn remote(scoop: &Scoop, paths: &[PathBuf], query: &str) -> Vec<Self> {
        let file = scoop
            .dir
            .join("apps")
            .join("scoop")
            .join("current")
            .join("buckets.json");
        let map: serde_json::Map<String, serde_json::Value> = match fs::read_to_string(file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(map) => map,
            None => return Vec::new(),
        };
        let mut buckets = Vec::new();
        for (name, url) in map {
            if paths
                .iter()
                .any(|p| p.file_name().is_some_and(|n| n == name.as_str()))
            {
                continue;
            }
            let Some(repo) = url
                .as_str()
                .and_then(|s| s.strip_prefix("https://github.com/"))
            else {
                continue;
            };
            let repo = repo.trim_end_matches('/').trim_end_matches(".git");
            if repo.split('/').count() != 2 {
                continue;
            }
            let url = format!(
                "https://api.github.com/repos/{}/git/trees/HEAD?recursive=1",
                repo
            );
            match App::search_remote_apps(&url, query) {
                Ok(apps) if !apps.is_empty() => buckets.push(Self { name, apps }),
                Ok(_) => {}
                Err(error) => eprintln!(
                    "Warning: could not search remote bucket '{}': {}",
                    name, error
                ),
            }
        }
        buckets.sort_by(|a, b| a.name.cmp(&b.name));
        buckets
    }
}
