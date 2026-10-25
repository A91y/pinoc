use anyhow::Result;
use clap::{Parser, Subcommand};

mod check;
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
        #[arg(
            long,
            help = "Include a worked PDA-account example instead of a no-op program"
        )]
        with_example: bool,
    },
    Build {
        #[arg(long, short, help = "Suppress verbose output")]
        quiet: bool,
        #[arg(
            long,
            short = 'F',
            value_delimiter = ',',
            help = "Space or comma separated list of features to activate (repeatable)"
        )]
        features: Vec<String>,
        #[arg(
            long,
            help = "Build for this `[clusters.<name>]` in Pinoc.toml, adding its features"
        )]
        cluster: Option<String>,
        #[arg(
            long,
            value_enum,
            help = "SBPF version to build for, passed to cargo build-sbf (default: [build].arch in Pinoc.toml, else the toolchain's)"
        )]
        arch: Option<config::Arch>,
        #[arg(
            long,
            help = "Program address override for IDL generation, for programs that don't use declare_id!"
        )]
        program_id: Option<String>,
        #[arg(
            long,
            value_enum,
            help = "Force the .codama.json generator instead of auto-detecting Codama macros"
        )]
        idl_generator: Option<Generator>,
    },
    Test {
        #[arg(long, short, help = "Suppress verbose output")]
        quiet: bool,
        #[arg(
            long,
            short = 'F',
            value_delimiter = ',',
            help = "Space or comma separated list of features to activate (repeatable)"
        )]
        features: Vec<String>,
        #[arg(
            long,
            help = "Build and test for this `[clusters.<name>]` in Pinoc.toml, adding its features"
        )]
        cluster: Option<String>,
        #[arg(
            long,
            conflicts_with_all = ["cluster", "no_build"],
            help = "Run the tests once for each `[clusters]` entry in Pinoc.toml (once per distinct feature set)"
        )]
        all_clusters: bool,
        #[arg(
            long,
            value_enum,
            help = "SBPF version to build for, passed to cargo build-sbf (default: [build].arch in Pinoc.toml, else the toolchain's)"
        )]
        arch: Option<config::Arch>,
        #[arg(
            long,
            help = "Skip the cargo build-sbf step and test against the existing target/deploy/*.so"
        )]
        no_build: bool,
        #[arg(
            long,
            value_delimiter = ',',
            help = "Features for the pre-test SBF build, if different from --features (\"\" for none)"
        )]
        build_features: Option<Vec<String>>,
        #[arg(
            last = true,
            value_name = "CARGO_ARGS",
            help = "Arguments for cargo test, e.g. `-- --test client -- --nocapture`"
        )]
        cargo_args: Vec<String>,
    },
    Deploy {
        #[arg(
            long,
            help = "A `[clusters.<name>]` in Pinoc.toml, or a URL or moniker for solana --url (default: [provider].cluster)"
        )]
        cluster: Option<String>,
        #[arg(long, help = "Wallet override")]
        wallet: Option<String>,
        #[arg(
            long,
            help = "The program's keypair (default: [provider].program_keypair, else target/deploy/<name>-keypair.json)"
        )]
        program_keypair: Option<String>,
        #[arg(
            long,
            help = "Upgrade authority keypair, passed to solana program deploy (default: from Pinoc.toml, else the wallet)"
        )]
        upgrade_authority: Option<String>,
    },
    /// Compare the deployed program with the local artifact.
    Verify {
        #[arg(
            long,
            help = "A `[clusters.<name>]` in Pinoc.toml, or a URL or moniker for solana --url (default: [provider].cluster)"
        )]
        cluster: Option<String>,
        #[arg(
            long,
            help = "The program's address (default: the one declared under src/)"
        )]
        program_id: Option<String>,
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
        #[arg(
            long,
            help = "Output directory for the IDL JSON [default: [idl].out_dir in Pinoc.toml, else target/idl]"
        )]
        out_dir: Option<String>,
        #[arg(
            long,
            help = "Program address to use, for programs that don't call declare_id!"
        )]
        program_id: Option<String>,
        #[arg(
            long,
            value_enum,
            help = "Force the .codama.json generator instead of auto-detecting Codama macros"
        )]
        idl_generator: Option<Generator>,
        #[arg(
            long,
            help = "Write nothing; exit nonzero if the IDL in the output directory differs from the source"
        )]
        check: bool,
    },
    Client {
        #[command(subcommand)]
        command: ClientCommands,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    Check {
        #[arg(long, help = "Emit findings as JSON")]
        json: bool,
        #[arg(
            long,
            num_args = 1..,
            help = "Promote these lint codes to deny; `*` or `all` for every code"
        )]
        deny: Vec<String>,
        #[arg(
            long,
            num_args = 1..,
            help = "Suppress these lint codes; `*` or `all` for every code"
        )]
        allow: Vec<String>,
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
        Commands::Build {
            quiet,
            features,
            cluster,
            arch,
            program_id,
            idl_generator,
        } => {
            commands::build::run_build(commands::build::BuildArgs {
                quiet: *quiet,
                features,
                cluster: cluster.as_deref(),
                arch: *arch,
                program_id: program_id.as_deref(),
                idl_generator: *idl_generator,
            })?;
        }
        Commands::Test {
            quiet,
            features,
            cluster,
            all_clusters,
            arch,
            no_build,
            build_features,
            cargo_args,
        } => {
            commands::test::run_test(commands::test::TestArgs {
                quiet: *quiet,
                features,
                build_features: build_features.as_deref(),
                cluster: cluster.as_deref(),
                all_clusters: *all_clusters,
                arch: *arch,
                no_build: *no_build,
                cargo_args,
            })?;
        }
        Commands::Deploy {
            cluster,
            wallet,
            program_keypair,
            upgrade_authority,
        } => {
            commands::deploy::run_deploy(commands::deploy::DeployArgs {
                cluster: cluster.as_deref(),
                wallet: wallet.as_deref(),
                program_keypair: program_keypair.as_deref(),
                upgrade_authority: upgrade_authority.as_deref(),
            })?;
        }
        Commands::Verify {
            cluster,
            program_id,
        } => {
            commands::deploy::run_verify(commands::deploy::VerifyArgs {
                cluster: cluster.as_deref(),
                program_id: program_id.as_deref(),
            })?;
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
        Commands::Idl {
            out_dir,
            program_id,
            idl_generator,
            check,
        } => {
            commands::idl::run_idl(
                out_dir.as_deref(),
                program_id.as_deref(),
                *idl_generator,
                *check,
            )?;
        }
        Commands::Client { command } => match command {
            ClientCommands::Generate {
                out_dir,
                idl_dir,
                generator,
                language,
                auto_install,
                yes,
                with_cpi,
                no_cpi,
                check,
            } => {
                commands::client::generate_client(commands::client::GenerateArgs {
                    idl_dir: idl_dir.as_deref(),
                    out_dir: out_dir.as_deref(),
                    generator: *generator,
                    language: *language,
                    check: *check,
                    auto_install: *auto_install,
                    yes: *yes,
                    with_cpi: *with_cpi,
                    no_cpi: *no_cpi,
                })?;
            }
        },
        Commands::Config { command } => match command {
            ConfigCommands::Init { yes } => {
                commands::config::init_config(*yes)?;
            }
        },
        Commands::Check { json, deny, allow } => {
            let code = commands::check::run_check(*json, deny.clone(), allow.clone())?;
            std::process::exit(code);
        }
        Commands::Help => {
            commands::help::display_help_banner()?;
        }
    }

    Ok(())
}
