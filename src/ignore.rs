use std::path::Path;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct IgnoreFilter {
    use_gitignore: bool,
    gitignore: Option<ignore::gitignore::Gitignore>,
    ignore_paths: Vec<String>,
}

impl IgnoreFilter {
    pub fn new(use_gitignore: bool, root_dir: &Path, ignore_paths: Vec<String>) -> Result<Self> {
        let gitignore = if use_gitignore {
            let mut builder = ignore::gitignore::GitignoreBuilder::new(root_dir);
            let gitignore_path = root_dir.join(".gitignore");
            if gitignore_path.exists() {
                builder.add(gitignore_path);
            }
            let _ = builder.add(root_dir.join(".git/info/exclude"));
            builder.build().ok()
        } else {
            None
        };

        Ok(IgnoreFilter {
            use_gitignore,
            gitignore,
            ignore_paths,
        })
    }

    pub fn is_ignored(&self, root: &Path, path: &Path) -> bool {
        if !self.use_gitignore {
            return false;
        }

        let relative = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => return false,
        };

        for p in &self.ignore_paths {
            let normalized = p.replace('\\', "/");
            let rel_str = relative.to_string_lossy().replace('\\', "/");
            if rel_str.starts_with(normalized.trim_end_matches('/'))
                || rel_str == normalized.trim_end_matches('/')
            {
                return true;
            }
        }

        if let Some(ref gi) = self.gitignore {
            if gi.matched(path, path.is_dir()).is_ignore() {
                return true;
            }
        }

        false
    }
}

pub fn is_git_dir(path: &Path) -> bool {
    path.file_name().map(|n| n == ".git").unwrap_or(false)
}

pub fn is_symlink(path: &Path) -> bool {
    path.is_symlink()
}

pub fn should_skip_dir(path: &Path) -> bool {
    is_git_dir(path)
}

pub fn should_skip_file(root: &Path, path: &Path, filter: &IgnoreFilter) -> bool {
    if is_symlink(path) {
        return true;
    }
    if filter.is_ignored(root, path) {
        return true;
    }
    false
}
