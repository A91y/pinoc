use shank_idl::idl::Idl;
use shank_idl::idl_type::IdlType;
use shank_idl::idl_type_definition::IdlTypeDefinitionTy;

/// Rewrites every `{"defined": "Address"}` type reference to the literal
/// `"publicKey"` IDL type. Needed because `@codama/nodes-from-anchor` mishandles
/// `Defined("Address")` (see docs/codama-comparison.md); pinoc's own generator
/// doesn't need this since it already understands that convention directly.
pub fn to_codama_compatible(idl: &Idl) -> Idl {
    let mut idl = idl.clone();

    for account in &mut idl.accounts {
        rewrite_type_definition(&mut account.ty);
    }
    for ty_def in &mut idl.types {
        rewrite_type_definition(&mut ty_def.ty);
    }
    for ix in &mut idl.instructions {
        for field in &mut ix.args {
            field.ty = rewrite_type(&field.ty);
        }
    }

    idl
}

fn rewrite_type_definition(ty: &mut IdlTypeDefinitionTy) {
    match ty {
        IdlTypeDefinitionTy::Struct { fields } => {
            for field in fields {
                field.ty = rewrite_type(&field.ty);
            }
        }
        IdlTypeDefinitionTy::Enum { .. } => {
            // pinoc's templates never emit enum accounts/types today.
        }
    }
}

fn rewrite_type(ty: &IdlType) -> IdlType {
    match ty {
        IdlType::Defined(name) if name == "Address" => IdlType::PublicKey,
        IdlType::Array(inner, size) => IdlType::Array(Box::new(rewrite_type(inner)), *size),
        IdlType::Vec(inner) => IdlType::Vec(Box::new(rewrite_type(inner))),
        IdlType::Option(inner) => IdlType::Option(Box::new(rewrite_type(inner))),
        other => other.clone(),
    }
}
