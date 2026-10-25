//! Marker comments above a `const`: `// pinoc:constant(u8)`,
//! `// pinoc:discriminators("event:", EventKind)`. Shared by IDL generation and
//! `pinoc check`, so "directly above" means the same thing to both.

use syn::spanned::Spanned;

/// The marker `name` on `line`: `Some(None)` for a bare `// name`,
/// `Some(Some(args))` for `// name(args)`, with `args` the text up to the last
/// `)`. The name has to end there, so `pinoc:discriminator` does not match
/// `// pinoc:discriminators(..)`.
pub fn marker_of<'a>(line: &'a str, name: &str) -> Option<Option<&'a str>> {
    let rest = line.trim().strip_prefix("//")?.trim().strip_prefix(name)?;
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(None);
    }
    let args = rest.strip_prefix('(')?;
    Some(Some(match args.rfind(')') {
        Some(end) => args[..end].trim(),
        None => args.trim(),
    }))
}

/// The marker `name` in the comment block directly above `item` (above its doc
/// comments and attributes, or between them and the `const` keyword), with the
/// 0-indexed line it is on.
pub fn marker_above<'a>(
    item: &syn::ItemConst,
    lines: &[&'a str],
    name: &str,
) -> Option<(usize, Option<&'a str>)> {
    let keyword = item.const_token.span().start().line.checked_sub(1)?;
    let mut top = item.span().start().line.checked_sub(1)?.min(keyword);
    while top > 0 && lines.get(top - 1)?.trim().starts_with("//") {
        top -= 1;
    }
    (top..keyword).find_map(|index| Some((index, marker_of(lines.get(index)?, name)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_name_does_not_match_a_longer_one() {
        let one = "pinoc:discriminator";
        let table = "pinoc:discriminators";
        let line = "// pinoc:discriminators(\"event:\", EventKind)";
        assert_eq!(marker_of(line, one), None);
        assert_eq!(marker_of(line, table), Some(Some("\"event:\", EventKind")));
        let line = "  //pinoc:discriminator( \"account:Vault\" ) ";
        assert_eq!(marker_of(line, one), Some(Some("\"account:Vault\"")));
        assert_eq!(marker_of(line, table), None);
        assert_eq!(marker_of("// pinoc:constant", "pinoc:constant"), Some(None));
        assert_eq!(marker_of("// pinoc:constants", "pinoc:constant"), None);
        assert_eq!(marker_of("// pinoc:constant u8", "pinoc:constant"), None);
    }
}
