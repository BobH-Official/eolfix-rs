mod convert;
mod ignore;
mod inspector;
mod rules;

use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::Parser;

use crate::convert::convert_file;
use crate::ignore::{should_skip_dir, should_skip_file, IgnoreFilter};
use crate::inspector::is_text;
use crate::rules::Rules;

#[derive(Parser, Debug)]
#[command(name = "no-crlf", version, about = "Normalize line endings in a directory tree")]
struct Cli {
    #[arg(value_name = "DIRECTORY", default_value = ".")]
    directory: PathBuf,

    #[arg(short = 'n', long, help = "Show what would be changed, don't write")]
    dry_run: bool,

    #[arg(short = 'v', long, help = "Print every file processed")]
    verbose: bool,

    #[arg(short = 'q', long, help = "Suppress non-error output")]
    quiet: bool,

    #[arg(long, help = "Only process top-level directory")]
    no_recursive: bool,

    #[arg(long, help = "Ignore .gitignore files")]
    no_gitignore: bool,

    #[arg(long = "config", value_name = "PATH", help = "Path to custom rule config file")]
    config: Option<PathBuf>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let root = cli
        .directory
        .canonicalize()
        .unwrap_or_else(|_| cli.directory.clone());

    if !root.is_dir() {
        anyhow::bail!("not a directory: {}", root.display());
    }

    let rules = Rules::load(cli.config.as_deref())?;

    let ignore_filter = IgnoreFilter::new(!cli.no_gitignore, &root, rules.ignore_paths.clone())?;

    let mut changed_count = 0u64;
    let mut skipped_count = 0u64;
    let mut ok_count = 0u64;
    let mut error_count = 0u64;

    let walker = if cli.no_recursive {
        walkdir::WalkDir::new(&root).max_depth(1)
    } else {
        walkdir::WalkDir::new(&root)
    };

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        let path = entry.path().to_path_buf();

        if path.is_dir() && should_skip_dir(&path) {
            continue;
        }

        if !path.is_file() {
            continue;
        }

        if should_skip_file(&root, &path, &ignore_filter) {
            if cli.verbose {
                println!("[SKIPPED] {}", path.display());
            }
            skipped_count += 1;
            continue;
        }

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if rules.is_ignored(&filename) {
            if cli.verbose {
                println!("[SKIPPED] {} (config ignore)", path.display());
            }
            skipped_count += 1;
            continue;
        }

        if !is_text(&path)? {
            if cli.verbose {
                println!("[SKIPPED] {} (binary)", path.display());
            }
            skipped_count += 1;
            continue;
        }

        let target = rules.determine_target(&filename);

        match convert_file(&path, target, cli.dry_run) {
            Ok(result) => {
                if result.error.is_some() {
                    if !cli.quiet {
                        eprintln!("error: {}: {}", path.display(), result.error.unwrap());
                    }
                    error_count += 1;
                } else if result.skipped {
                    if cli.verbose {
                        println!("[SKIPPED] {} (empty)", path.display());
                    }
                    skipped_count += 1;
                } else if result.changed {
                    if cli.verbose || cli.dry_run {
                        let prefix = if cli.dry_run { "(dry-run) " } else { "" };
                        println!("{}[CHANGED] {}", prefix, path.display());
                    }
                    changed_count += 1;
                } else {
                    if cli.verbose {
                        println!("[OK] {}", path.display());
                    }
                    ok_count += 1;
                }
            }
            Err(e) => {
                if !cli.quiet {
                    eprintln!("error: {}: {}", path.display(), e);
                }
                error_count += 1;
            }
        }
    }

    if !cli.quiet {
        println!(
            "{} files changed, {} files skipped, {} unchanged, {} errors",
            changed_count, skipped_count, ok_count, error_count
        );
    }

    Ok(())
}
