use anyhow::Result;
use clap::{Parser, Subcommand};

mod client_gen;
mod commands;
mod config;
mod idl;
mod templates;

use commands::client::ClientCommands;
use commands::config::ConfigCommands;
use commands::keys::KeyCommands;
use idl::Generator;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        project_name: String,
        #[arg(long, help = "Don't initialize git")]
        no_git: bool,
        #[arg(long, help = "Include a worked PDA-account example instead of a no-op program")]
        with_example: bool,
    },
    Build {
        #[arg(long, short, help = "Suppress verbose output")]
        quiet: bool,
        #[arg(
            long,
            help = "Program address override for IDL generation, for programs that don't use declare_id!"
        )]
        program_id: Option<String>,
        #[arg(long, value_enum, help = "Force the .codama.json generator instead of auto-detecting Codama macros")]
        idl_generator: Option<Generator>,
    },
    Test {
        #[arg(long, short, help = "Suppress verbose output")]
        quiet: bool,
    },
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
    Idl {
        #[arg(long, help = "Output directory for the IDL JSON", default_value = "target/idl")]
        out_dir: String,
        #[arg(long, help = "Program address to use, for programs that don't call declare_id!")]
        program_id: Option<String>,
        #[arg(long, value_enum, help = "Force the .codama.json generator instead of auto-detecting Codama macros")]
        idl_generator: Option<Generator>,
    },
    Client {
        #[command(subcommand)]
        command: ClientCommands,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    #[command(name = "--help")]
    Help,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init {
            project_name,
            no_git,
            with_example,
        } => {
            commands::init::init_project(project_name, *no_git, *with_example)?;
        }
        Commands::Build { quiet, program_id, idl_generator } => {
            commands::build::run_build(*quiet, program_id.as_deref(), *idl_generator)?;
        }
        Commands::Test { quiet } => {
            commands::test::run_test(*quiet)?;
        }
        Commands::Deploy { cluster, wallet } => {
            commands::deploy::run_deploy(cluster.as_deref(), wallet.as_deref())?;
        }
        Commands::Clean { no_preserve } => {
            commands::clean::clean_project(*no_preserve)?;
        }
        Commands::Add { package_name } => {
            commands::packages::add_package(package_name)?;
        }
        Commands::Search { query } => {
            commands::packages::search_packages(query.as_deref())?;
        }
        Commands::Keys { command } => match command {
            KeyCommands::List => {
                commands::keys::list_program_keys()?;
            }
            KeyCommands::Sync => {
                commands::keys::sync_program_keys()?;
            }
        },
        Commands::Idl { out_dir, program_id, idl_generator } => {
            commands::idl::run_idl(out_dir, program_id.as_deref(), *idl_generator)?;
        }
        Commands::Client { command } => match command {
            ClientCommands::Generate {
                out_dir,
                idl_dir,
                generator,
                auto_install,
                yes,
                with_cpi,
                no_cpi,
            } => {
                commands::client::generate_client(
                    idl_dir, out_dir.as_deref(), *generator, *auto_install, *yes, *with_cpi, *no_cpi,
                )?;
            }
        },
        Commands::Config { command } => match command {
            ConfigCommands::Init { yes } => {
                commands::config::init_config(*yes)?;
            }
        },
        Commands::Help => {
            commands::help::display_help_banner()?;
        }
    }

    Ok(())
}
