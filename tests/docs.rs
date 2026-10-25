//! Every relative link in the repository's markdown resolves: the file exists,
//! and a `#fragment` names a heading in it.

use std::path::{Path, PathBuf};

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if path.is_dir() {
            if !matches!(name, "target" | "node_modules" | ".git" | "fixtures") {
                markdown_files(&path, out);
            }
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
}

/// GitHub's heading anchor: lowercase, spaces to `-`, and everything but
/// letters, digits, `-` and `_` dropped.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

fn anchors(markdown: &str) -> Vec<String> {
    let mut in_code = false;
    let mut out = Vec::new();
    for line in markdown.lines() {
        if line.starts_with("```") {
            in_code = !in_code;
        } else if !in_code && line.starts_with('#') {
            out.push(anchor(line.trim_start_matches('#')));
        }
    }
    out
}

/// `[text](target)` targets outside code blocks and inline code.
fn links(markdown: &str) -> Vec<String> {
    let mut in_code = false;
    let mut out = Vec::new();
    for line in markdown.lines() {
        if line.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let line: String = line.split('`').step_by(2).collect::<Vec<_>>().join("");
        let mut rest = line.as_str();
        while let Some(at) = rest.find("](") {
            let after = &rest[at + 2..];
            let Some(end) = after.find(')') else { break };
            out.push(after[..end].to_string());
            rest = &after[end..];
        }
    }
    out
}

#[test]
fn every_relative_markdown_link_resolves() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    markdown_files(root, &mut files);
    let mut broken = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for link in links(&text) {
            if link.contains("://") || link.starts_with("mailto:") {
                continue;
            }
            let (path, fragment) = match link.split_once('#') {
                Some((p, f)) => (p, Some(f)),
                None => (link.as_str(), None),
            };
            let target = if path.is_empty() {
                file.clone()
            } else {
                file.parent().unwrap().join(path)
            };
            let shown = file.strip_prefix(root).unwrap().display();
            if !target.exists() {
                broken.push(format!("{shown}: {link} (no such file)"));
                continue;
            }
            if let Some(fragment) = fragment {
                let target_text = if target.is_dir() {
                    std::fs::read_to_string(target.join("README.md")).unwrap_or_default()
                } else {
                    std::fs::read_to_string(&target).unwrap_or_default()
                };
                if !anchors(&target_text).iter().any(|a| a == fragment) {
                    broken.push(format!("{shown}: {link} (no such heading)"));
                }
            }
        }
    }
    assert!(broken.is_empty(), "broken links:\n{}", broken.join("\n"));
}
