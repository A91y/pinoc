//! Where a finding is, for consumers that place it in a repository: GitHub
//! annotations and SARIF. Both need a path from the repository root and a
//! 1-based column, and neither takes a directory or line 0.

use crate::check::contract::Span;
use std::path::{Path, PathBuf};

pub struct Location {
    /// Relative to the repository root, `/`-separated.
    pub file: String,
    pub line: usize,
    /// 1-based.
    pub col: usize,
}

/// The finding's place in the repository. A project-wide finding (`src:0:0`)
/// goes on the crate's `Cargo.toml`, line 1, or nowhere when there is none.
pub fn locate(span: &Span) -> Option<Location> {
    let root = repo_root();
    if span.line == 0 {
        let manifest = Path::new("Cargo.toml");
        return manifest.exists().then(|| Location {
            file: relative(manifest, root.as_deref()),
            line: 1,
            col: 1,
        });
    }
    Some(Location {
        file: relative(Path::new(&span.file), root.as_deref()),
        line: span.line,
        col: span.col + 1,
    })
}

/// `GITHUB_WORKSPACE` when set, else the nearest directory holding `.git`.
fn repo_root() -> Option<PathBuf> {
    if let Some(workspace) = std::env::var_os("GITHUB_WORKSPACE") {
        return std::fs::canonicalize(workspace).ok();
    }
    let cwd = std::fs::canonicalize(".").ok()?;
    cwd.ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

fn relative(path: &Path, root: Option<&Path>) -> String {
    let from_root = root.and_then(|root| {
        let absolute = std::fs::canonicalize(path).ok()?;
        absolute.strip_prefix(root).ok().map(Path::to_path_buf)
    });
    let path = from_root.as_deref().unwrap_or(path);
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
