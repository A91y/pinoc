use crate::check::{self, CheckOptions, Format};
use anyhow::Result;

pub fn run_check(
    format: Format,
    explain: Option<String>,
    deny: Vec<String>,
    allow: Vec<String>,
) -> Result<i32> {
    check::run(CheckOptions {
        format,
        explain,
        deny,
        allow,
    })
}
