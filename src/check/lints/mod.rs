mod zc001_padding;
mod zc003_repr_c;

use crate::check::contract::{Lint, Span};
use syn::spanned::Spanned;

/// All registered lints.
pub fn registry() -> Vec<Box<dyn Lint>> {
    vec![
        Box::new(zc001_padding::Zc001Padding),
        Box::new(zc003_repr_c::Zc003ReprC),
    ]
}

/// Line is 1-indexed, column 0-indexed.
pub(crate) fn to_span(sp: proc_macro2::Span, file: &str) -> Span {
    let start = sp.start();
    Span {
        file: file.to_string(),
        line: start.line,
        col: start.column,
    }
}

/// Anchored at the first attribute so a `// pinoc:allow` directly above the item
/// suppresses the finding.
pub(crate) fn item_top_span(
    attrs: &[syn::Attribute],
    fallback: proc_macro2::Span,
) -> proc_macro2::Span {
    attrs.first().map(|a| a.span()).unwrap_or(fallback)
}
