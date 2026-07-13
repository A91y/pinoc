use anyhow::Result;

pub fn run_idl(out_dir: &str, program_id: Option<&str>) -> Result<()> {
    crate::idl::generate_idl(out_dir, program_id)
}
