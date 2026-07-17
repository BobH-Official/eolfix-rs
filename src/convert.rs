use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

use crate::rules::LineEnding;

const LARGE_FILE_THRESHOLD: u64 = 100 * 1024 * 1024;

pub struct ConvertResult {
    pub changed: bool,
    pub skipped: bool,
    pub error: Option<String>,
}

pub fn normalize_line_endings(content: &[u8], target: LineEnding) -> Vec<u8> {
    match target {
        LineEnding::Lf => {
            let step1 = replace_all(content, b"\r\n", b"\n");
            replace_all(&step1, b"\r", b"\n")
        }
        LineEnding::Crlf => {
            let step1 = replace_all(content, b"\r\n", b"\n");
            let step2 = replace_all(&step1, b"\r", b"\n");
            replace_all(&step2, b"\n", b"\r\n")
        }
        LineEnding::Cr => {
            let step1 = replace_all(content, b"\r\n", b"\n");
            let step2 = replace_all(&step1, b"\r", b"\n");
            replace_all(&step2, b"\n", b"\r")
        }
    }
}

fn replace_all(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    if data.is_empty() || from.is_empty() {
        return data.to_vec();
    }

    let mut result = Vec::with_capacity(data.len());
    let mut pos = 0;

    while pos < data.len() {
        if data[pos..].starts_with(from) {
            result.extend_from_slice(to);
            pos += from.len();
        } else {
            result.push(data[pos]);
            pos += 1;
        }
    }

    result
}

fn is_only_newline_endings(content: &[u8]) -> bool {
    if content.is_empty() {
        return true;
    }
    let last = content[content.len() - 1];
    last == b'\n' || last == b'\r'
}

pub fn convert_file(path: &Path, target: LineEnding, dry_run: bool) -> Result<ConvertResult> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            return Ok(ConvertResult {
                changed: false,
                skipped: true,
                error: Some(format!("cannot read metadata: {}", e)),
            });
        }
    };

    if metadata.len() == 0 {
        return Ok(ConvertResult {
            changed: false,
            skipped: true,
            error: None,
        });
    }

    if metadata.len() > LARGE_FILE_THRESHOLD {
        eprintln!(
            "warning: large file ({} MB): {}",
            metadata.len() / (1024 * 1024),
            path.display()
        );
    }

    let original = match fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            return Ok(ConvertResult {
                changed: false,
                skipped: true,
                error: Some(format!("cannot read file: {}", e)),
            });
        }
    };

    if !is_only_newline_endings(&original) && !original.is_empty() {
        return Ok(ConvertResult {
            changed: false,
            skipped: false,
            error: None,
        });
    }

    let normalized = normalize_line_endings(&original, target);

    if normalized == original {
        return Ok(ConvertResult {
            changed: false,
            skipped: false,
            error: None,
        });
    }

    if dry_run {
        return Ok(ConvertResult {
            changed: true,
            skipped: false,
            error: None,
        });
    }

    let dir = path.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("cannot create temp file for {}", path.display()))?;

    tmp.write_all(&normalized)
        .with_context(|| format!("cannot write temp file for {}", path.display()))?;

    tmp.persist(path)
        .with_context(|| format!("cannot persist temp file to {}", path.display()))?;

    Ok(ConvertResult {
        changed: true,
        skipped: false,
        error: None,
    })
}
