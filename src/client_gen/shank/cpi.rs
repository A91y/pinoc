//! CPI variants (`XxxCpi`/`XxxCpiAccounts`/`XxxCpiBuilder`), matching what the
//! real Codama Rust renderer emits for every instruction. Only generated when
//! the program itself appears to use CPI (or the user forces it), since it
//! pulls in 3 extra dependencies unconditionally otherwise unused.

use super::types::{idl_type_to_rust, safe_ident};
use anyhow::Result;
use heck::{ToPascalCase, ToSnakeCase};
use shank_idl::idl_instruction::{IdlAccount, IdlInstruction};
use std::path::Path;

/// Scans `.rs` files under `src_dir` for an `invoke(`/`invoke_signed(` call,
/// a decent signal the program participates in CPI. A false negative just
/// means the user passes `--with-cpi`; a false positive just generates
/// unused (harmless) code, so a plain text scan is enough here.
pub fn cpi_usage_detected(src_dir: &Path) -> Result<bool> {
    scan_dir(src_dir)
}

fn scan_dir(dir: &Path) -> Result<bool> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if scan_dir(&path)? {
                return Ok(true);
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        if contains_invoke_call(&src) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn contains_invoke_call(src: &str) -> bool {
    for needle in ["invoke(", "invoke_signed("] {
        if let Some(pos) = src.find(needle) {
            let before = src[..pos].chars().next_back();
            let is_word_boundary = before.map(|c| !c.is_alphanumeric() && c != '_').unwrap_or(true);
            if is_word_boundary {
                return true;
            }
        }
    }
    false
}

/// Generates the CPI variant block for one instruction, appended to the same
/// file as its plain (off-chain) builder. Reuses `{Name}InstructionArgs`,
/// already emitted by `instruction_rs`, instead of Codama's separate wrapper
/// struct (the wire format is identical either way).
pub fn instruction_cpi_rs(ix: &IdlInstruction, accounts: &[&IdlAccount]) -> Result<String> {
    let name = ix.name.to_pascal_case();

    let account_fields: String = accounts
        .iter()
        .map(|acc| {
            format!(
                "    pub {}: &'b solana_account_info::AccountInfo<'a>,\n",
                safe_ident(&acc.name.to_snake_case())
            )
        })
        .collect();

    let account_metas: String = accounts
        .iter()
        .map(|acc| {
            let acc_name = safe_ident(&acc.name.to_snake_case());
            let meta = if acc.is_mut {
                format!("solana_instruction::AccountMeta::new(*self.{acc_name}.key, {})", acc.is_signer)
            } else {
                format!(
                    "solana_instruction::AccountMeta::new_readonly(*self.{acc_name}.key, {})",
                    acc.is_signer
                )
            };
            format!("        accounts.push({meta});\n")
        })
        .collect();

    let account_info_pushes: String = accounts
        .iter()
        .map(|acc| format!("        account_infos.push(self.{}.clone());\n", safe_ident(&acc.name.to_snake_case())))
        .collect();

    let mut builder_setters = String::new();
    for acc in accounts {
        let acc_name = safe_ident(&acc.name.to_snake_case());
        builder_setters.push_str(&format!(
            "    pub fn {acc_name}(&mut self, {acc_name}: &'b solana_account_info::AccountInfo<'a>) -> &mut Self {{\n        self.instruction.{acc_name} = Some({acc_name});\n        self\n    }}\n"
        ));
    }
    let mut builder_arg_setters = String::new();
    for field in &ix.args {
        let field_name = safe_ident(&field.name.to_snake_case());
        let field_ty = idl_type_to_rust(&field.ty)?;
        builder_arg_setters.push_str(&format!(
            "    pub fn {field_name}(&mut self, {field_name}: {field_ty}) -> &mut Self {{\n        self.instruction.{field_name} = Some({field_name});\n        self\n    }}\n"
        ));
    }

    let account_count = accounts.len();
    let account_info_count = accounts.len() + 1; // + the target program's own AccountInfo

    let builder_account_fields: String = accounts
        .iter()
        .map(|acc| format!("    {}: Option<&'b solana_account_info::AccountInfo<'a>>,\n", safe_ident(&acc.name.to_snake_case())))
        .collect();
    let builder_arg_fields: String = ix
        .args
        .iter()
        .map(|f| Ok(format!("    {}: Option<{}>,\n", safe_ident(&f.name.to_snake_case()), idl_type_to_rust(&f.ty)?)))
        .collect::<Result<Vec<_>>>()?
        .join("");
    let builder_account_inits: String = accounts
        .iter()
        .map(|acc| format!("            {}: None,\n", safe_ident(&acc.name.to_snake_case())))
        .collect();
    let builder_arg_inits: String = ix
        .args
        .iter()
        .map(|f| format!("            {}: None,\n", safe_ident(&f.name.to_snake_case())))
        .collect();
    let builder_accounts_struct_fields: String = accounts
        .iter()
        .map(|acc| {
            let acc_name = safe_ident(&acc.name.to_snake_case());
            format!(
                "            {acc_name}: self.instruction.{acc_name}.expect(\"{acc_name} is required\"),\n"
            )
        })
        .collect();
    let builder_args_struct_fields: String = ix
        .args
        .iter()
        .map(|f| {
            let field_name = safe_ident(&f.name.to_snake_case());
            format!(
                "            {field_name}: self.instruction.{field_name}.clone().expect(\"{field_name} is required\"),\n"
            )
        })
        .collect();

    // A zero-account instruction has no fields referencing 'a/'b, so emitting an
    // empty `{name}CpiAccounts<'a, 'b>` would fail with E0392 (unused lifetime).
    // Match Codama: skip the accounts struct (and its `new` param) entirely.
    let cpi_accounts_struct = if accounts.is_empty() {
        String::new()
    } else {
        format!(
            "/// `{snake}` CPI accounts.\npub struct {name}CpiAccounts<'a, 'b> {{\n{account_fields}}}\n\n",
            snake = ix.name.to_snake_case(),
        )
    };
    let new_accounts_param = if accounts.is_empty() {
        String::new()
    } else {
        format!("        accounts: {name}CpiAccounts<'a, 'b>,\n")
    };

    Ok(format!(
        r#"
{cpi_accounts_struct}/// `{snake_name}` CPI instruction.
pub struct {name}Cpi<'a, 'b> {{
    pub __program: &'b solana_account_info::AccountInfo<'a>,
{account_fields}    pub __args: {name}InstructionArgs,
}}

impl<'a, 'b> {name}Cpi<'a, 'b> {{
    pub fn new(
        program: &'b solana_account_info::AccountInfo<'a>,
{new_accounts_param}        args: {name}InstructionArgs,
    ) -> Self {{
        Self {{
            __program: program,
{account_from_accounts}            __args: args,
        }}
    }}

    pub fn invoke(&self) -> solana_program_error::ProgramResult {{
        self.invoke_signed_with_remaining_accounts(&[], &[])
    }}

    pub fn invoke_with_remaining_accounts(
        &self,
        remaining_accounts: &[(&'b solana_account_info::AccountInfo<'a>, bool, bool)],
    ) -> solana_program_error::ProgramResult {{
        self.invoke_signed_with_remaining_accounts(&[], remaining_accounts)
    }}

    pub fn invoke_signed(&self, signers_seeds: &[&[&[u8]]]) -> solana_program_error::ProgramResult {{
        self.invoke_signed_with_remaining_accounts(signers_seeds, &[])
    }}

    pub fn invoke_signed_with_remaining_accounts(
        &self,
        signers_seeds: &[&[&[u8]]],
        remaining_accounts: &[(&'b solana_account_info::AccountInfo<'a>, bool, bool)],
    ) -> solana_program_error::ProgramResult {{
        let mut accounts = Vec::with_capacity({account_count} + remaining_accounts.len());
{account_metas}        remaining_accounts.iter().for_each(|ra| {{
            accounts.push(solana_instruction::AccountMeta {{
                pubkey: *ra.0.key,
                is_signer: ra.1,
                is_writable: ra.2,
            }});
        }});

        let mut data = vec![{upper_name}_DISCRIMINANT];
        data.extend(borsh::to_vec(&self.__args).expect("{name} args should always serialize"));

        let instruction = solana_instruction::Instruction {{
            program_id: crate::ID,
            accounts,
            data,
        }};

        let mut account_infos = Vec::with_capacity({account_info_count} + remaining_accounts.len());
        account_infos.push(self.__program.clone());
{account_info_pushes}        remaining_accounts.iter().for_each(|ra| account_infos.push(ra.0.clone()));

        if signers_seeds.is_empty() {{
            solana_cpi::invoke(&instruction, &account_infos)
        }} else {{
            solana_cpi::invoke_signed(&instruction, &account_infos, signers_seeds)
        }}
    }}
}}

#[derive(Default)]
struct {name}CpiBuilderInstruction<'a, 'b> {{
    __program: Option<&'b solana_account_info::AccountInfo<'a>>,
{builder_account_fields}{builder_arg_fields}}}

pub struct {name}CpiBuilder<'a, 'b> {{
    instruction: Box<{name}CpiBuilderInstruction<'a, 'b>>,
}}

impl<'a, 'b> {name}CpiBuilder<'a, 'b> {{
    pub fn new(program: &'b solana_account_info::AccountInfo<'a>) -> Self {{
        Self {{
            instruction: Box::new({name}CpiBuilderInstruction {{
                __program: Some(program),
{builder_account_inits}{builder_arg_inits}            }}),
        }}
    }}

{builder_setters}{builder_arg_setters}
    pub fn invoke(&self) -> solana_program_error::ProgramResult {{
        self.invoke_signed(&[])
    }}

    pub fn invoke_signed(&self, signers_seeds: &[&[&[u8]]]) -> solana_program_error::ProgramResult {{
        let cpi = {name}Cpi {{
            __program: self.instruction.__program.expect("program is required"),
{builder_accounts_struct_fields}            __args: {name}InstructionArgs {{
{builder_args_struct_fields}            }},
        }};
        cpi.invoke_signed(signers_seeds)
    }}
}}
"#,
        snake_name = ix.name.to_snake_case(),
        upper_name = ix.name.to_snake_case().to_uppercase(),
        account_from_accounts = accounts
            .iter()
            .map(|acc| {
                let acc_name = safe_ident(&acc.name.to_snake_case());
                format!("            {acc_name}: accounts.{acc_name},\n")
            })
            .collect::<String>(),
    ))
}
