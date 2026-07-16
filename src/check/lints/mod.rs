use crate::check::contract::Lint;

/// All registered lints.
pub fn registry() -> Vec<Box<dyn Lint>> {
    Vec::new()
}
