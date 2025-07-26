use clap::{Parser, Subcommand};

pub mod deploy;
pub use deploy::*;
pub mod build;
pub use build::*;
pub mod test;
pub use test::*;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum KeyCommands {
    List,
    Sync,
}

#[derive(Subcommand)]
pub enum Commands {
    Init {
        project_name: String,
        #[arg(long, help = "Don't initialize git")]
        no_git: bool,
        #[arg(long, help = "Create minimal project without tests and boilerplate")]
        no_boilerplate: bool,
    },
    Build,
    Test,
    Deploy {
        #[arg(long, help = "Cluster override")]
        cluster: Option<String>,
        #[arg(long, help = "Wallet override")]
        wallet: Option<String>,
    },
    Clean {
        #[arg(long, help = "Remove all files including keypair files")]
        no_preserve: bool,
    },
    Add {
        package_name: String,
    },
    Search {
        query: Option<String>,
    },
    Keys {
        #[command(subcommand)]
        command: KeyCommands,
    },
    #[command(name = "--help")]
    Help,
}
