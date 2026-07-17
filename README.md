# eolfix

Normalize line endings in a directory tree. Converts `CRLF` (`\r\n`) and `CR` (`\r`) to `LF` (`\n`) for all text files, with explicit "force rules" for file types that **must** use a specific line ending (e.g. `.bat` requires CRLF).

Uses **content inspection** (not extensions) to tell text from binary, so it handles files with no extension, ambiguous extensions (`.ts` = TypeScript or MPEG transport stream), and arbitrary filenames correctly.

---

## Features

- Recursive directory scanning (current dir or explicit path)
- Content-based text/binary detection — no reliance on file extensions
- Default conversion target: `LF`
- Force rules: certain file types (`.bat`, `.cmd`) always get `CRLF` even if they currently have `LF`
- Respects `.gitignore` rules (skips ignored files and directories)
- Always skips the `.git` directory
- Dry-run mode to preview changes without modifying files
- Configurable via `eolfix.toml` (or `.eolfix.toml`), with JSON Schema validation and built-in check/format commands

---

## Installation

```sh
cargo install eolfix
# or download from releases
```

---

## Usage

```
eolfix [OPTIONS] [DIRECTORY]

Arguments:
  DIRECTORY            Directory to scan (default: current directory)

Options:
  -n, --dry-run        Show what would be changed, don't write
  -v, --verbose        Print every file processed
  -q, --quiet          Suppress non-error output
      --no-recursive   Only process top-level directory
      --no-gitignore   Ignore .gitignore files
      --config <PATH>  Path to config file or directory containing config
      --check-config   Validate config file and exit
      --format-config  Print effective config as TOML and exit
      --help           Print help
      --version        Print version
```

### Examples

```sh
# Scan current directory, fix all line endings
eolfix

# Dry-run to see what would change
eolfix -n

# Scan a specific directory
eolfix /path/to/project

# Verbose dry-run
eolfix -vn /path/to/project

# Validate config file
eolfix --check-config
eolfix --check-config --config ~/.config/eolfix/

# Print effective config as TOML
eolfix --format-config
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

For each text file, consult the rules in priority order:

1. Match filename against `force_lf` patterns → target = `LF`
2. Match filename against `force_crlf` patterns → target = `CRLF`
3. Match filename against `force_cr` patterns → target = `CR`
4. Fallback → target = configured `default` (defaults to `LF`)

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

### Force LF

Files that **will break** with CRLF (shebang scripts, Makefiles):

| Pattern | Reason |
|---------|--------|
| `*.sh`, `*.bash`, `*.zsh`, `*.fish`, `*.ksh` | Shell scripts — `\r` in shebang causes "bad interpreter" |
| `*.mk`, `Makefile` | Makefiles — CRLF breaks tab-syntax parsing |

### Force CRLF

Files that **will break** if they don't have `\r\n`:

| Pattern | Reason |
|---------|--------|
| `*.bat` | Windows batch — fails with bare LF |
| `*.cmd` | Windows cmd script |

### Force CR

No built-in defaults. Legacy Mac (pre-OS X) used `\r`. Users can add via config.

---

## Configuration: `eolfix.toml`

Place in the scanned directory root (as `eolfix.toml` or `.eolfix.toml`), or pass via `--config <PATH>` (supports a directory or a file). Overrides/extend built-in defaults.

A [JSON Schema](./eolfix.schema.json) is provided for TOML validation. Add `"$schema"` to your config to enable IDE autocompletion and validation:

```toml
# eolfix.toml
"$schema" = "https://raw.githubusercontent.com/BobH-Official/eolfix/main/eolfix.schema.json"

# Default target line ending (optional, defaults to "lf")
default = "lf"

[force_lf]
patterns = ["*.sh", "*.bash", "*.zsh", "*.fish", "*.ksh", "*.mk", "Makefile"]
extensions = []
ignore_default = []

[force_crlf]
patterns = ["*.bat", "*.cmd"]
extensions = []

[force_cr]
patterns = []
extensions = []

[ignore]
# Additional ignore patterns (on top of .gitignore)
patterns = ["*.exe", "*.dll", "*.jpg", "*.png"]
paths = ["node_modules/", "target/", "dist/", "vendor/"]
```

### `--config` resolution

| `--config` value | Behavior |
|------------------|----------|
| Not provided | Search scanned directory for `eolfix.toml` → `.eolfix.toml` |
| A directory path | Search that directory for `eolfix.toml` → `.eolfix.toml` |
| A file path | Load that file directly |

### `--check-config` / `--format-config`

Use these to validate or inspect config before running:

```sh
eolfix --check-config                  # validate auto-discovered config
eolfix --check-config --config ./dir/  # validate config in directory
eolfix --format-config                 # print effective config as TOML
```

| Field | Type | Description |
|-------|------|-------------|
| `default` | `string` | Default target line ending: `"lf"`, `"crlf"`, or `"cr"` (default: `"lf"`) |
| `force_lf.patterns` | `string[]` | Glob patterns matching filenames that **must** use LF |
| `force_lf.extensions` | `string[]` | Extensions that **must** use LF (equivalent to `*.ext`) |
| `force_lf.ignore_default` | `string[]` | Extensions to **remove** from hardcoded LF default list |
| `force_crlf.patterns` | `string[]` | Glob patterns matching filenames that **must** use CRLF |
| `force_crlf.extensions` | `string[]` | Extensions that **must** use CRLF (equivalent to `*.ext`) |
| `force_crlf.ignore_default` | `string[]` | Extensions to **remove** from the hardcoded default list (e.g. `["bat"]` drops `*.bat`) |
| `force_cr.*` | same | Same, for CR |
| `ignore.patterns` | `string[]` | Glob patterns to skip entirely (matched against filename) |
| `ignore.paths` | `string[]` | Directory patterns to skip (matched against relative path prefix) |

**Merge behavior**: User config patterns/extensions are **appended** to the hardcoded list. Use `ignore_default` to remove specific hardcoded entries:

```toml
[force_crlf]
ignore_default = ["bat", "cmd"]   # remove all built-in defaults
patterns = ["*.sln", "*.vcxproj"] # only these get CRLF
```

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

`.gitattributes` controls line endings in Git's working tree. It doesn't fix existing files — it tells Git what to do on checkout/commit. `eolfix` is a one-shot fixer: it converts what's already on disk, independent of Git.

---

## License

MIT
