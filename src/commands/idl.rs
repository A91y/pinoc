use anyhow::{Context, Result};
use std::path::Path;

pub fn run_idl(
    out_dir: Option<&str>,
    program_id: Option<&str>,
    idl_generator: Option<crate::idl::Generator>,
    check: bool,
) -> Result<()> {
    let out_dir = match out_dir {
        Some(dir) => dir.to_string(),
        None => crate::config::idl_out_dir()?,
    };
    if !check {
        return crate::idl::generate_idl(&out_dir, program_id, idl_generator);
    }

    let fresh = std::env::temp_dir().join(format!("pinoc-idl-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&fresh);
    let generated = fresh
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid path: {}", fresh.display()))
        .and_then(|dir| crate::idl::generate_idl(dir, program_id, idl_generator));
    let stale = generated.and_then(|()| stale_files(&fresh, Path::new(&out_dir)));
    let _ = std::fs::remove_dir_all(&fresh);
    report_stale(
        &stale?,
        &format!("The IDL in {out_dir}/"),
        "Run `pinoc idl` and commit the result.",
    )
}

/// The files under `fresh` that `committed` lacks or holds with other contents,
/// as paths relative to both.
pub fn stale_files(fresh: &Path, committed: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    collect(fresh, fresh, &mut files)?;
    files.sort();
    let mut stale = Vec::new();
    for rel in files {
        let new = std::fs::read(fresh.join(&rel))
            .with_context(|| format!("Failed to read {}", fresh.join(&rel).display()))?;
        match std::fs::read(committed.join(&rel)) {
            Ok(old) if old == new => {}
            Ok(_) => stale.push(format!("{} (differs)", rel.display())),
            Err(_) => stale.push(format!("{} (missing)", rel.display())),
        }
    }
    Ok(stale)
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, root, out)?;
        } else {
            out.push(path.strip_prefix(root)?.to_path_buf());
        }
    }
    Ok(())
}

/// Ok when nothing is stale; otherwise an error listing what is.
pub fn report_stale(stale: &[String], what: &str, fix: &str) -> Result<()> {
    if stale.is_empty() {
        println!("✅ {what} is up to date");
        return Ok(());
    }
    anyhow::bail!(
        "{what} is out of date:\n{}\n\n{fix}",
        stale
            .iter()
            .map(|s| format!("   {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
