use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, ParsedFile, Severity};
use crate::check::lints::to_span;
use heck::ToSnakeCase;
use sha2::{Digest, Sha256};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, Item, ItemConst, Lit, Token};

/// A discriminator constant that is not the hash it claims to be. The marker
/// above a `const` names the preimage, and the bytes must be the first bytes of
/// its SHA-256 (the Anchor convention: `event:Name`, `account:Name`,
/// `global:snake_name`):
///
/// ```text
/// // pinoc:discriminator("event:VaultDeposit")
/// const VAULT_DEPOSIT: [u8; 8] = [4, 248, 234, 163, 99, 238, 140, 45];
///
/// // pinoc:discriminators("event:", EventKind)
/// const DISCRIMINATORS: [[u8; 8]; 25] = [ .. ];
/// ```
///
/// In the table form, row `i` is checked against the variant of the enum whose
/// discriminant is `i`, so the enum and the table cannot drift apart. A third
/// argument, `snake_case`, hashes the variant name in snake case.
pub struct Disc001Discriminator;

const MARKER: &str = "pinoc:discriminator";

impl Lint for Disc001Discriminator {
    fn code(&self) -> &'static str {
        "DISC001-P"
    }
    fn id(&self) -> &'static str {
        "wrong-discriminator"
    }
    fn category(&self) -> Category {
        Category::Disc
    }
    fn backend(&self) -> Backend {
        Backend::Syn
    }
    fn default_severity(&self) -> Severity {
        Severity::Deny
    }
    fn default_confidence(&self) -> Confidence {
        Confidence::Definite
    }

    fn run(&self, file: &ParsedFile) -> Vec<Finding> {
        let path = file.path.display().to_string();
        let lines: Vec<&str> = file.src.lines().collect();
        let mut consts = Vec::new();
        collect_consts(&file.ast.items, &mut consts);

        let mut out = Vec::new();
        let mut finding = |span: proc_macro2::Span, evidence: String, fix: Option<String>| {
            out.push(Finding {
                code: self.code(),
                id: self.id(),
                confidence: self.default_confidence(),
                severity: self.default_severity(),
                span: to_span(span, &path),
                evidence,
                fix,
            });
        };

        for item in consts {
            let Some(marker) = marker_above(item, &lines) else {
                continue;
            };
            let name = &item.ident;
            let rows = match check_const(item, &marker, &file.ast.items) {
                Ok(rows) => rows,
                Err(reason) => {
                    finding(
                        item.const_token.span(),
                        format!("`// {MARKER}..` above `{name}` cannot be checked: {reason}"),
                        None,
                    );
                    continue;
                }
            };
            for row in rows {
                if row.actual != row.expected {
                    finding(
                        row.span,
                        format!(
                            "{} is {}, but sha256({:?})[..{}] is {}",
                            row.what,
                            bytes(&row.actual),
                            row.preimage,
                            row.expected.len(),
                            bytes(&row.expected)
                        ),
                        Some(format!("replace it with {}", bytes(&row.expected))),
                    );
                }
            }
        }
        out
    }
}

struct Row {
    what: String,
    span: proc_macro2::Span,
    preimage: String,
    actual: Vec<u8>,
    expected: Vec<u8>,
}

enum Marker {
    /// `pinoc:discriminator("<preimage>")`
    One(String),
    /// `pinoc:discriminators("<prefix>", Enum[, snake_case])`
    Table {
        prefix: String,
        enum_name: String,
        snake: bool,
    },
}

fn check_const(item: &ItemConst, marker: &str, items: &[Item]) -> Result<Vec<Row>, String> {
    let marker = parse_marker(marker)?;
    let name = item.ident.to_string();
    match marker {
        Marker::One(preimage) => {
            let actual = byte_array(&item.expr)?;
            Ok(vec![Row {
                what: format!("`{name}`"),
                span: item.expr.span(),
                expected: digest(&preimage, actual.len())?,
                preimage,
                actual,
            }])
        }
        Marker::Table {
            prefix,
            enum_name,
            snake,
        } => {
            let Expr::Array(table) = &*item.expr else {
                return Err("its value is not an array of byte arrays".to_string());
            };
            let variants = enum_variants(items, &enum_name)?;
            if variants.len() != table.elems.len() {
                return Err(format!(
                    "`{enum_name}` has {} variants and the table {} rows",
                    variants.len(),
                    table.elems.len()
                ));
            }
            let mut rows = Vec::new();
            for (index, row) in table.elems.iter().enumerate() {
                let Some((variant, _)) = variants.iter().find(|(_, d)| *d == index as i128) else {
                    return Err(format!(
                        "no variant of `{enum_name}` has discriminant {index}, so row {index} names nothing"
                    ));
                };
                let actual = byte_array(row)?;
                let shown = if snake {
                    variant.to_snake_case()
                } else {
                    variant.clone()
                };
                let preimage = format!("{prefix}{shown}");
                rows.push(Row {
                    what: format!("`{name}[{index}]` (`{enum_name}::{variant}`)"),
                    span: row.span(),
                    expected: digest(&preimage, actual.len())?,
                    preimage,
                    actual,
                });
            }
            Ok(rows)
        }
    }
}

fn parse_marker(rest: &str) -> Result<Marker, String> {
    let usage = format!("expected `{MARKER}(\"<preimage>\")` or `{MARKER}s(\"<prefix>\", Enum)`");
    let (table, args) = if let Some(args) = rest.strip_prefix("s(") {
        (true, args)
    } else if let Some(args) = rest.strip_prefix('(') {
        (false, args)
    } else {
        return Err(usage);
    };
    let args = args.strip_suffix(')').ok_or_else(|| usage.clone())?;
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let args: Vec<Expr> = syn::parse::Parser::parse_str(parser, args)
        .map_err(|_| usage.clone())?
        .into_iter()
        .collect();
    let string = |e: &Expr| match e {
        Expr::Lit(syn::ExprLit {
            lit: Lit::Str(s), ..
        }) => Some(s.value()),
        _ => None,
    };
    let ident = |e: &Expr| match e {
        Expr::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    };
    match (table, args.as_slice()) {
        (false, [preimage]) => string(preimage).map(Marker::One).ok_or(usage),
        (true, [prefix, enum_name]) => Ok(Marker::Table {
            prefix: string(prefix).ok_or_else(|| usage.clone())?,
            enum_name: ident(enum_name).ok_or(usage)?,
            snake: false,
        }),
        (true, [prefix, enum_name, case]) if ident(case).as_deref() == Some("snake_case") => {
            Ok(Marker::Table {
                prefix: string(prefix).ok_or_else(|| usage.clone())?,
                enum_name: ident(enum_name).ok_or(usage)?,
                snake: true,
            })
        }
        _ => Err(usage),
    }
}

/// `enum_name`'s variants in this file with their discriminants: the declared
/// integer, else one more than the previous variant's.
fn enum_variants(items: &[Item], enum_name: &str) -> Result<Vec<(String, i128)>, String> {
    let Some(e) = find_enum(items, enum_name) else {
        return Err(format!(
            "there is no `enum {enum_name}` in this file; the table and its enum have to be in the same file"
        ));
    };
    let mut next = 0i128;
    let mut out = Vec::new();
    for variant in &e.variants {
        let value = match &variant.discriminant {
            None => next,
            Some((
                _,
                Expr::Lit(syn::ExprLit {
                    lit: Lit::Int(i), ..
                }),
            )) => i
                .base10_parse::<i128>()
                .map_err(|_| format!("`{}`'s discriminant is not an integer", variant.ident))?,
            Some(_) => {
                return Err(format!(
                    "`{}`'s discriminant is not an integer literal",
                    variant.ident
                ))
            }
        };
        out.push((variant.ident.to_string(), value));
        next = value + 1;
    }
    Ok(out)
}

fn find_enum<'a>(items: &'a [Item], name: &str) -> Option<&'a syn::ItemEnum> {
    items.iter().find_map(|item| match item {
        Item::Enum(e) if e.ident == name => Some(e),
        Item::Mod(m) => m
            .content
            .as_ref()
            .and_then(|(_, inner)| find_enum(inner, name)),
        _ => None,
    })
}

fn byte_array(expr: &Expr) -> Result<Vec<u8>, String> {
    let Expr::Array(array) = expr else {
        return Err("a value is not an array literal of bytes".to_string());
    };
    array
        .elems
        .iter()
        .map(|e| match e {
            Expr::Lit(syn::ExprLit {
                lit: Lit::Int(i), ..
            }) => i
                .base10_parse::<u8>()
                .map_err(|_| format!("`{i}` is not a byte")),
            _ => Err("an element is not an integer literal".to_string()),
        })
        .collect()
}

fn digest(preimage: &str, len: usize) -> Result<Vec<u8>, String> {
    if len == 0 || len > 32 {
        return Err(format!(
            "a discriminator of {len} bytes cannot be a prefix of a SHA-256"
        ));
    }
    Ok(Sha256::digest(preimage.as_bytes())[..len].to_vec())
}

fn bytes(b: &[u8]) -> String {
    format!(
        "[{}]",
        b.iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn collect_consts<'a>(items: &'a [Item], out: &mut Vec<&'a ItemConst>) {
    for item in items {
        match item {
            Item::Const(c) => out.push(c),
            Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_consts(inner, out);
                }
            }
            _ => {}
        }
    }
}

/// The text after `pinoc:discriminator` in a `//` comment directly above the
/// const (above its doc comments and attributes, or among them).
fn marker_above(item: &ItemConst, lines: &[&str]) -> Option<String> {
    let keyword = item.const_token.span().start().line.checked_sub(1)?;
    let mut top = item.span().start().line.checked_sub(1)?.min(keyword);
    while top > 0 && lines.get(top - 1)?.trim().starts_with("//") {
        top -= 1;
    }
    (top..keyword).find_map(|index| {
        let comment = lines.get(index)?.trim().strip_prefix("//")?.trim();
        comment
            .strip_prefix(MARKER)
            .map(|rest| rest.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn findings(src: &str) -> Vec<Finding> {
        let file = ParsedFile {
            path: PathBuf::from("src/events.rs"),
            src: src.to_string(),
            ast: syn::parse_file(src).unwrap(),
        };
        Disc001Discriminator.run(&file)
    }

    fn row(preimage: &str) -> String {
        bytes(&Sha256::digest(preimage.as_bytes())[..8])
    }

    #[test]
    fn a_correct_table_and_constant_pass() {
        let src = format!(
            "pub enum EventKind {{ Deposit = 0, Withdraw = 1, Close }}\n\
             // pinoc:discriminators(\"event:\", EventKind)\n\
             const T: [[u8; 8]; 3] = [{}, {}, {}];\n\
             /// The account's.\n\
             // pinoc:discriminator(\"account:Vault\")\n\
             #[allow(dead_code)]\n\
             const V: [u8; 8] = {};\n",
            row("event:Deposit"),
            row("event:Withdraw"),
            row("event:Close"),
            row("account:Vault")
        );
        assert!(findings(&src).is_empty());
    }

    #[test]
    fn a_wrong_row_is_reported_at_its_line_with_the_right_bytes() {
        let src = format!(
            "pub enum EventKind {{ Deposit, Withdraw }}\n\
             // pinoc:discriminators(\"event:\", EventKind)\n\
             const T: [[u8; 8]; 2] = [\n    {},\n    {},\n];\n",
            row("event:Deposit"),
            row("event:Withdrawn")
        );
        let found = findings(&src);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].span.line, 5);
        assert!(found[0]
            .evidence
            .starts_with("`T[1]` (`EventKind::Withdraw`) is"));
        assert_eq!(
            found[0].fix.as_deref(),
            Some(format!("replace it with {}", row("event:Withdraw")).as_str())
        );
    }

    #[test]
    fn rows_follow_explicit_discriminants_and_snake_case() {
        let src = format!(
            "enum Ix {{ OpenVault = 1, Close = 0 }}\n\
             // pinoc:discriminators(\"global:\", Ix, snake_case)\n\
             const T: [[u8; 8]; 2] = [{}, {}];\n",
            row("global:close"),
            row("global:open_vault")
        );
        assert!(findings(&src).is_empty());
    }

    #[test]
    fn a_marker_that_cannot_be_checked_says_why() {
        for (src, why) in [
            (
                "// pinoc:discriminators(\"event:\", Missing)\nconst T: [[u8; 8]; 0] = [];\n",
                "there is no `enum Missing` in this file",
            ),
            (
                "enum E { A, B }\n// pinoc:discriminators(\"event:\", E)\nconst T: [[u8; 8]; 1] = [[0; 8]];\n",
                "`E` has 2 variants and the table 1 rows",
            ),
            (
                "// pinoc:discriminator(event)\nconst T: [u8; 8] = [0; 8];\n",
                "expected `pinoc:discriminator(\"<preimage>\")`",
            ),
        ] {
            let found = findings(src);
            assert_eq!(found.len(), 1, "{src}");
            assert!(found[0].evidence.contains(why), "{}", found[0].evidence);
        }
    }
}
