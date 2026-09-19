use nucleo_matcher::{Config, Matcher, Utf32Str, Utf32String};
use std::cell::RefCell;

// Nucleo's scratch space is expensive to create. Reuse it per search worker,
// with no shared mutex on the parallel matching path.
thread_local! {
    static NUCLEO: RefCell<(Matcher, Vec<char>)> = RefCell::new({
        let mut config = Config::DEFAULT;
        config.ignore_case = false; // Both query and candidate are lowercased.
        config.normalize = false;
        config.prefer_prefix = true;
        (Matcher::new(config), Vec::new())
    });
}

/// Lower scores sort first: literal, separator-insensitive, typo, subsequence.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Score(u8, usize, usize);

pub(crate) struct Query {
    text: String,
    normalized: String,
    chars: Vec<char>,
    needle: Utf32String,
}

impl Query {
    pub fn new(text: &str) -> Self {
        let text = text.to_lowercase();
        let normalized = normalize(&text);
        let chars = normalized.chars().collect();
        let needle = Utf32String::from(text.as_str());
        Self {
            text,
            normalized,
            chars,
            needle,
        }
    }

    pub fn score(&self, candidate: &str) -> Option<Score> {
        let candidate = candidate.to_lowercase();
        let length = candidate.chars().count();
        if candidate == self.text {
            return Some(Score(0, 0, 0));
        }
        if candidate.contains(&self.text) {
            return Some(Score(
                if candidate.starts_with(&self.text) {
                    1
                } else {
                    2
                },
                0,
                length,
            ));
        }
        if self.normalized.is_empty() {
            return None;
        }
        let normalized = normalize(&candidate);
        if normalized == self.normalized {
            return Some(Score(3, 0, 0));
        }
        // Preserve conservative spelling correction, including transpositions.
        if (3..=128).contains(&self.chars.len()) {
            let limit = if self.chars.len() >= 6 { 2 } else { 1 };
            let delta = self.chars.len().abs_diff(normalized.chars().count());
            if delta <= limit {
                let distance = strsim::osa_distance(&self.normalized, &normalized);
                if distance <= limit {
                    return Some(Score(4, distance, delta));
                }
            }
        }
        // Two-letter abbreviations are useful (rg -> ripgrep). A single letter
        // or punctuation-only query should not introduce additional noise.
        if !(2..=128).contains(&self.needle.len()) || !self.text.chars().any(char::is_alphanumeric)
        {
            return None;
        }
        NUCLEO.with(|state| {
            let mut state = state.borrow_mut();
            let (matcher, buffer) = &mut *state;
            matcher
                .fuzzy_match(Utf32Str::new(&candidate, buffer), self.needle.slice(..))
                // Nucleo uses higher-is-better scores; our ordering is ascending.
                .map(|score| Score(5, usize::from(u16::MAX - score), length))
        })
    }
}

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '-' | '_' | ' '))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_common_typos_and_unicode() {
        for (query, name) in [
            ("gti", "git"),
            ("ripgrp", "ripgrep"),
            ("ripgreep", "ripgrep"),
            ("firefpx", "firefox"),
            ("ÉDITUER", "éditeur"),
            ("visualstudiocode", "visual-studio-code"),
        ] {
            assert!(
                Query::new(query).score(name).is_some(),
                "{} -> {}",
                query,
                name
            );
        }
    }
    #[test]
    fn rejects_noise_but_preserves_literal_queries() {
        for (query, name) in [
            ("tg", "git"),
            ("git", "python"),
            ("---", "git"),
            ("c++", "cpp"),
        ] {
            assert!(Query::new(query).score(name).is_none());
        }
        assert!(Query::new("").score("anything").is_some());
        assert!(
            Query::new("git").score("git").unwrap() < Query::new("git").score("git-lfs").unwrap()
        );
        assert!(
            Query::new("git").score("git-lfs").unwrap() < Query::new("git").score("legit").unwrap()
        );
    }

    #[test]
    fn nucleo_matches_ordered_abbreviations_and_scores_them() {
        for (query, name) in [
            ("rg", "ripgrep"),
            ("vsc", "visual-studio-code"),
            ("RG", "RipGrep"),
            ("编器", "文字编辑器"),
        ] {
            assert!(
                Query::new(query).score(name).is_some(),
                "{} -> {}",
                query,
                name
            );
        }
        assert!(Query::new("vrsc").score("visual-studio-code").is_none());
        let query = Query::new("vsc");
        assert!(
            query.score("visual-studio-code").unwrap()
                < query.score("very-long-something-code").unwrap()
        );
        assert!(Query::new("rg").score("rg").unwrap() < Query::new("rg").score("ripgrep").unwrap());
    }
}
