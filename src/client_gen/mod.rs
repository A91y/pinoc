pub mod codama;
pub mod shank;

/// Target language of the generated client. Only the codama generator renders TypeScript.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Language {
    Rust,
    Ts,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Language::Rust => "Rust",
            Language::Ts => "TypeScript",
        }
    }
}
