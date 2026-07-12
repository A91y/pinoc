pub fn cargo_toml(project_name: &str) -> String {
    format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
pinocchio = "0.11.2"
solana-address = {{ version = "2.6.1", features = ["decode"] }}
"#,
        project_name
    )
}

pub fn lib_rs(program_address: &str) -> String {
    let template = r#"use pinocchio::{AccountView, Address, ProgramResult};

pub const ID: Address = Address::from_str_const("{program_address}");

pub fn process_instruction(
    _program_id: &Address,
    _accounts: &mut [AccountView],
    _instruction_data: &[u8],
) -> ProgramResult {
    // Your program logic here
    Ok(())
}
"#;

    template.replace("{program_address}", program_address)
}

pub fn readme_md(project_name: &str) -> String {
    format!(
        r#"# {}

A minimal Solana program built with Pinocchio.

## Building

```bash
pinoc build
```

## Deployment

```bash
pinoc deploy
```
"#,
        project_name
    )
}
