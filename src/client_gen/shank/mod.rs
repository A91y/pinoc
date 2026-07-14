mod accounts;
pub mod cpi;
mod instructions;
mod manifest;
mod shared;
mod types;

use accounts::account_rs;
use anyhow::{Context, Result};
use heck::ToSnakeCase;
use instructions::instruction_rs;
use manifest::{cargo_toml, lib_rs};
use shank_idl::idl::Idl;
use shared::shared_rs;
use std::fs;
use std::path::Path;
use types::{safe_ident, type_def_rs};

/// Mirrors the shape of Codama's Rust renderer output, not the literal renderer itself.
pub fn generate_rust_client(idl_path: &Path, out_dir: &Path, generate_cpi: bool) -> Result<()> {
    let idl_json = fs::read_to_string(idl_path)
        .with_context(|| format!("Failed to read IDL at {}", idl_path.display()))?;
    let idl: Idl = serde_json::from_str(&idl_json).with_context(|| "Failed to parse IDL JSON")?;

    let program_address = idl
        .metadata
        .address
        .clone()
        .ok_or_else(|| anyhow::anyhow!("IDL metadata has no program address"))?;

    let src_dir = out_dir.join("src");
    let generated_dir = src_dir.join("generated");
    let accounts_dir = generated_dir.join("accounts");
    let instructions_dir = generated_dir.join("instructions");
    let types_dir = generated_dir.join("types");
    fs::create_dir_all(&accounts_dir)?;
    fs::create_dir_all(&instructions_dir)?;
    fs::create_dir_all(&types_dir)?;

    fs::write(out_dir.join("Cargo.toml"), cargo_toml(&idl.name, generate_cpi))?;
    fs::write(src_dir.join("lib.rs"), lib_rs(&program_address))?;
    fs::write(generated_dir.join("shared.rs"), shared_rs())?;

    let mut type_mods = Vec::new();
    for ty_def in &idl.types {
        let file_name = ty_def.name.to_snake_case();
        fs::write(
            types_dir.join(format!("{file_name}.rs")),
            type_def_rs(ty_def)?,
        )?;
        type_mods.push(file_name);
    }
    fs::write(types_dir.join("mod.rs"), mod_rs(&type_mods))?;

    let mut account_mods = Vec::new();
    for account in &idl.accounts {
        let file_name = account.name.to_snake_case();
        fs::write(
            accounts_dir.join(format!("{file_name}.rs")),
            account_rs(account)?,
        )?;
        account_mods.push(file_name);
    }
    fs::write(accounts_dir.join("mod.rs"), mod_rs(&account_mods))?;

    let mut ix_mods = Vec::new();
    for ix in &idl.instructions {
        let file_name = ix.name.to_snake_case();
        fs::write(
            instructions_dir.join(format!("{file_name}.rs")),
            instruction_rs(ix, generate_cpi)?,
        )?;
        ix_mods.push(file_name);
    }
    fs::write(instructions_dir.join("mod.rs"), mod_rs(&ix_mods))?;

    fs::write(
        generated_dir.join("mod.rs"),
        "pub mod accounts;\npub mod instructions;\npub mod shared;\npub mod types;\n\npub use accounts::*;\npub use instructions::*;\npub use shared::*;\npub use types::*;\n",
    )?;

    Ok(())
}

fn mod_rs(files: &[String]) -> String {
    let mut out = String::new();
    for f in files {
        let m = safe_ident(f);
        out.push_str(&format!("pub mod {m};\npub use {m}::*;\n"));
    }
    out
}
