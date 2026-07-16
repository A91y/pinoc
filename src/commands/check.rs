use crate::check::{self, CheckOptions};
use anyhow::Result;

pub fn run_check(json: bool, deny: Vec<String>, allow: Vec<String>) -> Result<i32> {
    check::run(CheckOptions { json, deny, allow })
}
