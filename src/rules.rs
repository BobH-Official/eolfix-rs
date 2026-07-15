use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    LF,
    CRLF,
    CR,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ForceRulesSection {
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IgnoreSection {
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RuleConfig {
    #[serde(default)]
    pub force_crlf: ForceRulesSection,
    #[serde(default)]
    pub force_cr: ForceRulesSection,
    #[serde(default)]
    pub ignore: IgnoreSection,
}

#[derive(Debug, Clone)]
pub struct Rules {
    pub force_crlf_patterns: Vec<String>,
    pub force_cr_patterns: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub ignore_paths: Vec<String>,
}

const HARDCODED_FORCE_CRLF_EXTENSIONS: &[&str] = &["bat", "cmd", "ps1", "psm1", "psd1"];

impl Rules {
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let mut force_crlf_patterns: Vec<String> = HARDCODED_FORCE_CRLF_EXTENSIONS
            .iter()
            .map(|ext| format!("*.{}", ext))
            .collect();
        let mut force_cr_patterns: Vec<String> = Vec::new();
        let mut ignore_patterns: Vec<String> = Vec::new();
        let mut ignore_paths: Vec<String> = Vec::new();

        if let Some(path) = config_path {
            let content = std::fs::read_to_string(path)?;
            let config: RuleConfig = toml::from_str(&content)?;

            force_crlf_patterns.extend(config.force_crlf.patterns);
            for ext in &config.force_crlf.extensions {
                let ext = ext.trim_start_matches('.');
                force_crlf_patterns.push(format!("*.{}", ext));
            }

            force_cr_patterns.extend(config.force_cr.patterns);
            for ext in &config.force_cr.extensions {
                let ext = ext.trim_start_matches('.');
                force_cr_patterns.push(format!("*.{}", ext));
            }

            ignore_patterns.extend(config.ignore.patterns);
            ignore_paths.extend(config.ignore.paths);
        }

        Ok(Rules {
            force_crlf_patterns,
            force_cr_patterns,
            ignore_patterns,
            ignore_paths,
        })
    }

    pub fn determine_target(&self, filename: &str) -> LineEnding {
        if matches_any_pattern(filename, &self.force_crlf_patterns) {
            return LineEnding::CRLF;
        }
        if matches_any_pattern(filename, &self.force_cr_patterns) {
            return LineEnding::CR;
        }
        LineEnding::LF
    }

    pub fn is_ignored(&self, filename: &str) -> bool {
        matches_any_pattern(filename, &self.ignore_patterns)
    }
}

fn matches_any_pattern(filename: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| {
        glob::Pattern::new(p)
            .map(|pat| pat.matches(filename))
            .unwrap_or(false)
    })
}
