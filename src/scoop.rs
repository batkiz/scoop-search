use std::{env, error::Error, fs, path::PathBuf};

#[derive(Debug, PartialEq)]
pub struct Scoop {
    pub dir: PathBuf,
    pub buckets_dir: PathBuf,
}

impl Scoop {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Self::resolve(
            env::var_os("SCOOP").map(PathBuf::from),
            env::var_os("USERPROFILE").map(PathBuf::from),
            env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        )
    }

    pub(crate) fn resolve(
        scoop: Option<PathBuf>,
        home: Option<PathBuf>,
        config_home: Option<PathBuf>,
    ) -> Result<Self, Box<dyn Error>> {
        let nonempty = |p: &PathBuf| !p.as_os_str().is_empty();
        let home = home.filter(nonempty);
        let dir = if let Some(dir) = scoop.filter(nonempty) {
            dir
        } else {
            let config_home = config_home
                .filter(nonempty)
                .or_else(|| home.as_ref().map(|p| p.join(".config")));
            let configured = config_home
                .and_then(|p| fs::read_to_string(p.join("scoop").join("config.json")).ok())
                .and_then(|text| Self::configured_root(&text));
            match configured {
                Some(dir) => dir,
                None => home
                    .ok_or("Cannot locate Scoop: set SCOOP or USERPROFILE")?
                    .join("scoop"),
            }
        };
        Ok(Self {
            buckets_dir: dir.join("buckets"),
            dir,
        })
    }

    fn configured_root(text: &str) -> Option<PathBuf> {
        let config: serde_json::Value = serde_json::from_str(text).ok()?;
        ["root_path", "rootPath"].iter().find_map(|key| {
            config
                .get(key)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_paths_are_unquoted_and_prefer_current_key() {
        assert_eq!(
            Scoop::configured_root(r#"{"root_path":"D:\\My Scoop","rootPath":"old"}"#),
            Some(PathBuf::from(r"D:\My Scoop"))
        );
        assert_eq!(
            Scoop::configured_root(r#"{"rootPath":"legacy"}"#),
            Some(PathBuf::from("legacy"))
        );
    }
    #[test]
    fn explicit_root_wins_and_missing_home_is_an_error() {
        assert_eq!(
            Scoop::resolve(Some("custom".into()), None, None)
                .unwrap()
                .dir,
            PathBuf::from("custom")
        );
        assert!(Scoop::resolve(None, None, None).is_err());
        assert_eq!(
            Scoop::resolve(None, Some("home".into()), None).unwrap().dir,
            PathBuf::from("home").join("scoop")
        );
    }
}
