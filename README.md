# No-Crlf

Normalize line endings in a directory tree. Converts `CRLF` (`\r\n`) and `CR` (`\r`) to `LF` (`\n`) for all text files, with explicit "force rules" for file types that **must** use a specific line ending (e.g. `.bat` requires CRLF).

Uses **content inspection** (not extensions) to tell text from binary, so it handles files with no extension, ambiguous extensions (`.ts` = TypeScript or MPEG transport stream), and arbitrary filenames correctly.

---

## Features

- Recursive directory scanning (current dir or explicit path)
- Content-based text/binary detection — no reliance on file extensions
- Default conversion target: `LF`
- Force rules: certain file types (`.bat`, `.cmd`, `.ps1`, etc.) always get `CRLF` even if they currently have `LF`
- Respects `.gitignore` rules (skips ignored files and directories)
- Always skips the `.git` directory
- Dry-run mode to preview changes without modifying files
- Configurable via `crlf-rules.toml`

---

## Installation

```sh
cargo install no-crlf
# or download from releases
```

---

## Usage

```
no-crlf [OPTIONS] [DIRECTORY]

Arguments:
  DIRECTORY            Directory to scan (default: current directory)

Options:
  -n, --dry-run        Show what would be changed, don't write
  -v, --verbose        Print every file processed
  -q, --quiet          Suppress non-error output
      --no-recursive   Only process top-level directory
      --no-gitignore   Ignore .gitignore files
      --config <PATH>  Path to custom rule config file
      --help           Print help
      --version        Print version
```

### Examples

```sh
# Scan current directory, fix all line endings
no-crlf

# Dry-run to see what would change
no-crlf -n

# Scan a specific directory
no-crlf /path/to/project

# Verbose dry-run
no-crlf -vn /path/to/project
```

---

## How It Works

### 1. Collect files

Walk the directory tree recursively. For each file:

- Skip anything inside `.git/`
- Skip files/directories matched by `.gitignore` rules (unless `--no-gitignore`)
- Follow symlinks? **No** — symlinks are skipped.

### 2. Text vs binary detection (content inspection)

Read the **first 8 KiB** of each file and apply two checks:

| Check | Rule | Rationale |
|-------|------|-----------|
| Null byte | If file contains `0x00` → **binary** | Null bytes almost never appear in text files |
| Printable ratio | If `(control_chars / sampled_bytes) > 0.10` → **binary** | Binary files have high density of non-printable bytes |

**Whitelist bytes** (not counted as "control chars"): `\t`, `\n`, `\r`, `\x1b` (ESC, for ANSI sequences).

If either check triggers, the file is skipped.

### 3. Determine target line ending

For each text file, consult the "force rules" in order:

1. Match filename against `force_crlf` patterns → target = `CRLF`
2. Match filename against `force_cr` patterns → target = `CR`
3. Default → target = `LF`

Force rules use **filename-only matching** (not full path) against glob patterns and extension lists.

### 4. Convert

- Read the **entire file** into memory
- Normalize all line endings to the target:
  - `\r\n` → `\n`, then `\n` → `\r\n` (if CRLF target)
  - `\r` → `\n`, then `\n` → `\r` (if CR target)
  - `\r\n` → `\n`, `\r` → `\n` (if LF target)
- If the content is unchanged, skip the write
- Write atomically (write to temp file, then rename) to avoid corruption on interruption

### 5. Report

| Mode | Output |
|------|--------|
| Default | Summary: "X files changed, Y files skipped, Z errors" |
| `--verbose` | Per-file: `[CHANGED] path` / `[SKIPPED] path (binary)` / `[OK] path` |
| `--quiet` | Only errors to stderr |
| `--dry-run` | Same output but with `(dry-run)` prefix, no files modified |

---

## Default Force Rules (hardcoded)

### Force CRLF

Files that **will break** if they don't have `\r\n`:

| Pattern / Extension | Reason |
|---------------------|--------|
| `*.bat` | Windows batch — fails with bare LF |
| `*.cmd` | Windows cmd script |
| `*.ps1` | PowerShell script |
| `*.psm1` | PowerShell module |
| `*.psd1` | PowerShell data file |

### Force CR

No built-in defaults. Legacy Mac (pre-OS X) used `\r`. Users can add via config.

---

## Configuration: `crlf-rules.toml`

Place in the scanned directory root. Overrides/extend built-in defaults.

```toml
# These MERGE with (don't replace) hardcoded defaults

[force_crlf]
patterns = ["*.bat", "*.cmd", "*.ps1", "*.psm1", "*.psd1"]
extensions = [".sln", ".vcxproj"]

[force_cr]
patterns = []
extensions = []

[ignore]
# Additional ignore patterns (on top of .gitignore)
patterns = ["*.exe", "*.dll", "*.jpg", "*.png"]
paths = ["node_modules/", "target/", "dist/", "vendor/"]
```

| Field | Type | Description |
|-------|------|-------------|
| `force_crlf.patterns` | `string[]` | Glob patterns matching filenames that **must** use CRLF |
| `force_crlf.extensions` | `string[]` | Extensions that **must** use CRLF (equivalent to `*.ext`) |
| `force_cr.*` | same | Same, for CR |
| `ignore.patterns` | `string[]` | Glob patterns to skip entirely (matched against filename) |
| `ignore.paths` | `string[]` | Directory patterns to skip (matched against relative path prefix) |

**Merge behavior**: User config patterns/extensions are **appended** to the hardcoded list, not a replacement. To remove a hardcoded rule, the user would need to configure the list explicitly (future enhancement).

---

## Edge Cases & Error Handling

| Scenario | Behavior |
|----------|----------|
| Empty file (0 bytes) | Skip — nothing to convert |
| File with only one line (no newline at EOF) | Leave as-is (no line ending to convert) |
| File with mixed line endings (`\r\n` and `\r` and `\n`) | Normalize all to target line ending |
| Very large file (> 100 MB) | Warn on stderr, still process |
| Permission denied (can't read) | Warn on stderr, skip |
| File locked (can't write) | Warn on stderr, skip |
| Symlink | Skip (don't follow) |
| UTF-8 BOM | **Preserve** the BOM, only modify line endings |
| File already has correct line endings | Skip write (no-op), report as `[OK]` |
| Binary misidentified as text | Threshold is conservative (~10% control chars). If a binary slips through, the write would corrupt it. This is why `--dry-run` is important for first use. |
| File with `\r\n` but force rule says CRLF | Already correct → no-op |
| File with `\n` but force rule says CRLF | Convert to `\r\n` |

---

## Dependencies (Rust)

| Crate | Purpose |
|-------|---------|
| `clap` (derive) | CLI argument parsing |
| `walkdir` | Recursive directory walking |
| `ignore` | `.gitignore` parsing and matching (from ripgrep) |
| `toml` | Config file parsing |
| `glob` | Pattern matching for force rules |
| `tempfile` | Atomic write via temp file + rename |
| `anyhow` | Error handling ergonomics |

---

## Why Not `.gitattributes`?

`.gitattributes` controls line endings in Git's working tree. It doesn't fix existing files — it tells Git what to do on checkout/commit. `no-crlf` is a one-shot fixer: it converts what's already on disk, independent of Git.

---

## License

MIT
