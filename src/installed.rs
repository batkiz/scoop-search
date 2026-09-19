use crate::scoop::Scoop;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

struct Record {
    bucket: Option<String>,
    version: String,
}

pub(crate) struct Installed {
    local: HashMap<String, Record>,
    global: HashMap<String, Record>,
}

fn json(path: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

impl Installed {
    pub fn load(scoop: &Scoop) -> Self {
        let config_home = env::var_os("XDG_CONFIG_HOME")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join(".config")));
        let config = config_home
            .and_then(|p| json(&p.join("scoop/config.json")))
            .unwrap_or_default();
        let global = Self::global_root(
            env::var_os("SCOOP_GLOBAL").map(PathBuf::from),
            &config,
            env::var_os("ProgramData").map(PathBuf::from),
        );
        let no_junction = config
            .get("no_junction")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Self::from_roots(&scoop.dir, global.as_deref(), no_junction)
    }

    fn global_root(
        explicit: Option<PathBuf>,
        config: &serde_json::Value,
        program_data: Option<PathBuf>,
    ) -> Option<PathBuf> {
        explicit
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| {
                config
                    .get("global_path")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
            })
            .or_else(|| {
                program_data
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(|p| p.join("scoop"))
            })
    }

    pub(crate) fn from_roots(local: &Path, global: Option<&Path>, no_junction: bool) -> Self {
        Self {
            local: Self::scan(local, no_junction),
            global: global
                .map(|p| Self::scan(p, no_junction))
                .unwrap_or_default(),
        }
    }

    fn read_record(dir: &Path) -> Option<Record> {
        let info = json(&dir.join("install.json"))?;
        info.as_object()?;
        let manifest = json(&dir.join("manifest.json"))?;
        let version = manifest.get("version")?.as_str()?.to_owned();
        if version.is_empty() {
            return None;
        }
        let bucket = info
            .get("bucket")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase);
        Some(Record { bucket, version })
    }

    fn scan(root: &Path, no_junction: bool) -> HashMap<String, Record> {
        let mut records = HashMap::new();
        let Ok(apps) = fs::read_dir(root.join("apps")) else {
            return records;
        };
        for app in apps.flatten() {
            if !app.path().is_dir() {
                continue;
            }
            let mut record = if no_junction {
                None
            } else {
                Self::read_record(&app.path().join("current"))
            };
            if record.is_none() {
                // Scoop falls back to the newest install.json when junctions
                // are disabled or current is missing. Ignore backup directories.
                if let Ok(entries) = fs::read_dir(app.path()) {
                    let mut versions: Vec<_> = entries
                        .flatten()
                        .filter(|e| {
                            let name = e.file_name().to_string_lossy().into_owned();
                            name != "current"
                                && !(name.starts_with('_') && name.contains(".old"))
                                && e.path().is_dir()
                        })
                        .filter_map(|e| {
                            let modified = fs::metadata(e.path().join("install.json"))
                                .ok()?
                                .modified()
                                .ok()?;
                            Some((modified, e.path()))
                        })
                        .collect();
                    versions.sort();
                    record = versions
                        .last()
                        .and_then(|(_, path)| Self::read_record(path));
                }
            }
            if let Some(record) = record {
                records.insert(app.file_name().to_string_lossy().to_lowercase(), record);
            }
        }
        records
    }

    pub fn label(&self, bucket: &str, name: &str) -> String {
        let mut result = String::new();
        let name = name.to_lowercase();
        for (records, scope) in [
            (&self.local, "installed"),
            (&self.global, "installed globally"),
        ] {
            if let Some(record) = records.get(&name) {
                if record
                    .bucket
                    .as_ref()
                    .is_some_and(|source| !source.eq_ignore_ascii_case(bucket))
                {
                    continue;
                }
                let source = if record.bucket.is_none() {
                    " (source unknown)"
                } else {
                    ""
                };
                result.push_str(&format!(" [{}{}: {}]", scope, source, record.version));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_path_precedence() {
        let config = serde_json::json!({"global_path":"configured"});
        assert_eq!(
            Installed::global_root(Some("explicit".into()), &config, Some("data".into())),
            Some("explicit".into())
        );
        assert_eq!(
            Installed::global_root(None, &config, Some("data".into())),
            Some("configured".into())
        );
        assert_eq!(
            Installed::global_root(None, &serde_json::Value::Null, Some("data".into())),
            Some(PathBuf::from("data").join("scoop"))
        );
    }
}
