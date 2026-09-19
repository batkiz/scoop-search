use crate::fuzzy::{Query, Score};
use std::{error::Error, fs, path::Path};

#[derive(Debug, PartialEq, Clone)]
pub struct App {
    pub name: String,
    pub version: String,
    pub bin: Vec<String>,
}

impl App {
    pub fn read(path: &Path) -> Option<Self> {
        Self::parse(path.file_stem()?.to_str()?, &fs::read_to_string(path).ok()?)
    }

    fn parse(name: &str, content: &str) -> Option<Self> {
        let manifest: serde_json::Value = serde_json::from_str(content).ok()?;
        let manifest = manifest.as_object()?;
        let version = match manifest.get("version") {
            Some(v) => v.as_str()?.to_owned(),
            None => String::new(),
        };
        let mut bin = Vec::new();
        match manifest.get("bin") {
            Some(serde_json::Value::String(v)) => bin.push(v.clone()),
            Some(serde_json::Value::Array(values)) => {
                for v in values {
                    match v {
                        serde_json::Value::String(v) => bin.push(v.clone()),
                        serde_json::Value::Array(v) => {
                            // Executable and alias only, never command arguments.
                            bin.extend(
                                v.iter()
                                    .take(2)
                                    .filter_map(|v| v.as_str())
                                    .map(str::to_owned),
                            );
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Some(Self {
            name: name.to_owned(),
            version,
            bin,
        })
    }

    pub fn matching(mut self, query: &str, names_only: bool) -> Option<Self> {
        if self.name.to_lowercase().contains(query) {
            self.bin.clear();
            return Some(self);
        }
        if names_only {
            return None;
        }
        self.bin = self
            .bin
            .into_iter()
            .map(|bin| bin.rsplit(['/', '\\']).next().unwrap_or("").to_owned())
            .filter(|bin| bin.to_lowercase().contains(query))
            .collect();
        self.bin.sort();
        self.bin.dedup();
        if self.bin.is_empty() {
            None
        } else {
            Some(self)
        }
    }

    pub fn fuzzy_matching(mut self, query: &Query, names_only: bool) -> Option<(Score, Self)> {
        let name_score = query.score(&self.name);
        let mut bins = Vec::new();
        if !names_only {
            for bin in &self.bin {
                let name = bin.rsplit(['/', '\\']).next().unwrap_or("");
                let lower = name.to_lowercase();
                let stem = [".exe", ".cmd", ".bat", ".ps1", ".com", ".sh"]
                    .iter()
                    .find_map(|ext| lower.strip_suffix(ext));
                let score = query
                    .score(name)
                    .into_iter()
                    .chain(stem.and_then(|s| query.score(s)))
                    .min();
                if let Some(score) = score {
                    bins.push((score, name.to_owned()));
                }
            }
        }
        bins.sort();
        bins.dedup();
        let bin_score = bins.first().map(|(score, _)| *score);
        let score = name_score.into_iter().chain(bin_score).min()?;
        self.bin = if name_score == Some(score) {
            Vec::new()
        } else {
            bins.into_iter().map(|(_, name)| name).collect()
        };
        Some((score, self))
    }

    pub fn remote_apps(url: &str) -> Result<Vec<App>, Box<dyn Error>> {
        let response = ureq::get(url)
            .timeout_connect(5_000)
            .timeout_read(10_000)
            .call();
        if response.status() != 200 {
            return Err(format!("GitHub request failed: HTTP {}", response.status()).into());
        }
        Self::remote_matches(&response.into_json()?, "")
    }

    fn remote_matches(
        response: &serde_json::Value,
        query: &str,
    ) -> Result<Vec<App>, Box<dyn Error>> {
        let tree = response
            .get("tree")
            .and_then(|v| v.as_array())
            .ok_or("Missing repository tree")?;
        let mut apps: Vec<_> = tree
            .iter()
            .filter(|e| e["type"] == "blob")
            .filter_map(|e| e["path"].as_str())
            .filter(|p| p.starts_with("bucket/") || !p.contains('/'))
            .filter_map(|p| p.strip_suffix(".json"))
            .filter_map(|p| p.rsplit('/').next())
            .filter(|name| name.to_lowercase().contains(query))
            .map(|name| App {
                name: name.to_owned(),
                version: String::new(),
                bin: Vec::new(),
            })
            .collect();
        apps.sort_by(|a, b| a.name.cmp(&b.name));
        apps.dedup_by(|a, b| a.name == b.name);
        Ok(apps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_match_once_and_arguments_do_not_match() {
        let app = App::parse(
            "tool",
            r#"{"version":"1","bin":[["dir\\FOO.exe","foo","--secret"],"other/foo.exe"]}"#,
        )
        .unwrap();
        assert!(app.clone().matching("secret", false).is_none());
        assert!(app.clone().matching("foo", true).is_none());
        assert_eq!(app.matching("foo", false).unwrap().bin.len(), 3);
    }
    #[test]
    fn invalid_manifests_do_not_panic() {
        for content in ["invalid", "[]", r#"{"version":42}"#] {
            assert!(App::parse("tool", content).is_none());
        }
    }
    #[test]
    fn remote_results_are_sorted_package_names() {
        let response = serde_json::json!({"tree": [
            {"type":"blob", "path":"bucket/git.json"},
            {"type":"blob", "path":"scripts/git.json"},
            {"type":"tree", "path":"fake.json"},
            {"type":"blob", "path":"7zip.json"}
        ]});
        let apps = App::remote_matches(&response, "").unwrap();
        assert_eq!(
            apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            ["7zip", "git"]
        );
    }
}
