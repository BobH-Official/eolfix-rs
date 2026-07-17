use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
    Cr,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ForceRulesSection {
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub ignore_default: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct IgnoreSection {
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleConfig {
    #[serde(default = "default_default")]
    pub default: String,

    #[serde(default)]
    pub force_lf: ForceRulesSection,
    #[serde(default)]
    pub force_crlf: ForceRulesSection,
    #[serde(default)]
    pub force_cr: ForceRulesSection,
    #[serde(default)]
    pub ignore: IgnoreSection,
}

fn default_default() -> String {
    "lf".to_string()
}

#[derive(Debug, Clone)]
pub struct Rules {
    pub default_line_ending: LineEnding,
    pub force_lf_patterns: Vec<String>,
    pub force_crlf_patterns: Vec<String>,
    pub force_cr_patterns: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub ignore_paths: Vec<String>,
}

const HARDCODED_FORCE_CRLF_PATTERNS: &[&str] = &["*.bat", "*.cmd"];

const HARDCODED_FORCE_LF_PATTERNS: &[&str] = &[
    "*.sh", "*.bash", "*.zsh", "*.fish", "*.ksh", "*.mk", "Makefile",
];

const ALL_CONFIG_NAMES: &[&str] = &["eolfix.toml", ".eolfix.toml"];

fn parse_line_ending(s: &str) -> Result<LineEnding> {
    match s.to_lowercase().as_str() {
        "lf" => Ok(LineEnding::Lf),
        "crlf" => Ok(LineEnding::Crlf),
        "cr" => Ok(LineEnding::Cr),
        _ => anyhow::bail!("invalid line ending '{}': must be 'lf', 'crlf', or 'cr'", s),
    }
}

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

fn merge_rules_section(defaults: &[&str], config: &ForceRulesSection, target: &mut Vec<String>) {
    target.extend(defaults.iter().map(|s| s.to_string()));

    for ignore in &config.ignore_default {
        let p = ignore.trim_start_matches('*').trim_start_matches('.');
        let pattern = format!("*.{}", p);
        target.retain(|x| x != &pattern && x != ignore);
    }

    target.extend(config.patterns.iter().cloned());
    for ext in &config.extensions {
        let ext = ext.trim_start_matches('.');
        target.push(format!("*.{}", ext));
    }
}

impl Rules {
    pub fn load(root_dir: &Path, config_path: Option<&Path>) -> Result<Self> {
        let mut force_lf_patterns: Vec<String> = Vec::new();
        let mut force_crlf_patterns: Vec<String> = Vec::new();
        let mut force_cr_patterns: Vec<String> = Vec::new();
        let mut ignore_patterns: Vec<String> = Vec::new();
        let mut ignore_paths: Vec<String> = Vec::new();

        let mut default = LineEnding::Lf;

        if let Some(path) = resolve_config_path(root_dir, config_path) {
            let config = load_config_file(&path)?;

            default = parse_line_ending(&config.default)?;

            merge_rules_section(
                HARDCODED_FORCE_LF_PATTERNS,
                &config.force_lf,
                &mut force_lf_patterns,
            );
            merge_rules_section(
                HARDCODED_FORCE_CRLF_PATTERNS,
                &config.force_crlf,
                &mut force_crlf_patterns,
            );
            merge_rules_section(&[], &config.force_cr, &mut force_cr_patterns);

            ignore_patterns.extend(config.ignore.patterns);
            ignore_paths.extend(config.ignore.paths);
        } else {
            force_lf_patterns.extend(HARDCODED_FORCE_LF_PATTERNS.iter().map(|s| s.to_string()));
            force_crlf_patterns.extend(HARDCODED_FORCE_CRLF_PATTERNS.iter().map(|s| s.to_string()));
        }

        Ok(Rules {
            default_line_ending: default,
            force_lf_patterns,
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
        parse_line_ending(&config.default)?;

        for p in config
            .force_lf
            .patterns
            .iter()
            .chain(&config.force_crlf.patterns)
            .chain(&config.force_cr.patterns)
            .chain(&config.ignore.patterns)
        {
            glob::Pattern::new(p)
                .map_err(|e| anyhow::anyhow!("invalid glob pattern '{}': {}", p, e))?;
        }

        for p in &config.ignore.paths {
            if p.is_empty() {
                anyhow::bail!("ignore.paths contains an empty string");
            }
        }

        println!("config OK (default = {})", config.default);
        Ok(())
    }

    pub fn format_config(root_dir: &Path, config_path: Option<&Path>) -> Result<String> {
        let config = match resolve_config_path(root_dir, config_path) {
            Some(path) => load_config_file(&path)?,
            None => {
                let mut c = RuleConfig {
                    default: "lf".into(),
                    force_lf: ForceRulesSection::default(),
                    force_crlf: ForceRulesSection::default(),
                    force_cr: ForceRulesSection::default(),
                    ignore: IgnoreSection::default(),
                };
                c.force_lf.patterns = HARDCODED_FORCE_LF_PATTERNS
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                c.force_crlf.patterns = HARDCODED_FORCE_CRLF_PATTERNS
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                c
            }
        };

        Ok(toml::to_string(&config)?)
    }

    pub fn determine_target(&self, filename: &str) -> LineEnding {
        if matches_any_pattern(filename, &self.force_lf_patterns) {
            return LineEnding::Lf;
        }
        if matches_any_pattern(filename, &self.force_crlf_patterns) {
            return LineEnding::Crlf;
        }
        if matches_any_pattern(filename, &self.force_cr_patterns) {
            return LineEnding::Cr;
        }
        self.default_line_ending
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
