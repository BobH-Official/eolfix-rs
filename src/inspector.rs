use std::path::Path;

use anyhow::Result;

const SAMPLE_SIZE: usize = 8192;

const WHITELIST_CONTROL_BYTES: &[u8] = &[b'\t', b'\n', b'\r', 0x1b];

pub fn is_text(path: &Path) -> Result<bool> {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("warning: cannot read {}: {}", path.display(), e);
            return Ok(false);
        }
    };

    let mut buf = vec![0u8; SAMPLE_SIZE];
    let n = {
        use std::io::Read;
        file.read(&mut buf).unwrap_or(0)
    };

    if n == 0 {
        return Ok(false);
    }

    let sample = &buf[..n];

    if sample.contains(&0x00) {
        return Ok(false);
    }

    let control_count = sample
        .iter()
        .filter(|&&b| b < 0x20 && !WHITELIST_CONTROL_BYTES.contains(&b))
        .count();

    let ratio = control_count as f64 / n as f64;

    if ratio > 0.10 {
        return Ok(false);
    }

    Ok(true)
}
