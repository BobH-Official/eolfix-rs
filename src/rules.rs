use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    LF,
    CRLF,
    CR,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ForceRulesSection {
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct IgnoreSection {
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
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

const HARDCODED_FORCE_CRLF_EXTENSIONS: &[&str] = &["bat", "cmd"];

const ALL_CONFIG_NAMES: &[&str] = &["eolfix.toml", ".eolfix.toml"];

fn search_config_in_dir(dir: &Path) -> Option<PathBuf> {
    ALL_CONFIG_NAMES.iter().find_map(|name| {
        let path = dir.join(name);
        if path.is_file() { Some(path) } else { None }
    })
}

pub fn resolve_config_path(root_dir: &Path, user_path: Option<&Path>) -> Option<PathBuf> {
    match user_path {
        Some(p) if p.is_dir() => search_config_in_dir(p),
        Some(p) => Some(p.to_path_buf()),
        None => search_config_in_dir(root_dir),
    }
}

fn load_config_file(path: &Path) -> Result<RuleConfig> {
    let content = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

impl Rules {
    pub fn load(root_dir: &Path, config_path: Option<&Path>) -> Result<Self> {
        let mut force_crlf_patterns: Vec<String> = HARDCODED_FORCE_CRLF_EXTENSIONS
            .iter()
            .map(|ext| format!("*.{}", ext))
            .collect();
        let mut force_cr_patterns: Vec<String> = Vec::new();
        let mut ignore_patterns: Vec<String> = Vec::new();
        let mut ignore_paths: Vec<String> = Vec::new();

        if let Some(path) = resolve_config_path(root_dir, config_path) {
            let config = load_config_file(&path)?;

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

    pub fn check_config(root_dir: &Path, config_path: Option<&Path>) -> Result<()> {
        let path = match resolve_config_path(root_dir, config_path) {
            Some(p) => p,
            None => {
                println!("no config file found (eolfix.toml / .eolfix.toml), using defaults");
                return Ok(());
            }
        };

        println!("config file: {}", path.display());

        let config = load_config_file(&path)?;

        for p in config
            .force_crlf
            .patterns
            .iter()
            .chain(&config.force_cr.patterns)
            .chain(&config.ignore.patterns)
        {
            glob::Pattern::new(p).map_err(|e| anyhow::anyhow!("invalid glob pattern '{}': {}", p, e))?;
        }

        for p in &config.ignore.paths {
            if p.is_empty() {
                anyhow::bail!("ignore.paths contains an empty string");
            }
        }

        println!("config OK");
        Ok(())
    }

    pub fn format_config(root_dir: &Path, config_path: Option<&Path>) -> Result<String> {
        let config = match resolve_config_path(root_dir, config_path) {
            Some(path) => load_config_file(&path)?,
            None => RuleConfig::default(),
        };

        Ok(toml::to_string(&config)?)
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
