use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;

#[derive(Debug, Parser)]
#[command(name = "rs-infra-style")]
struct Cli {
    #[arg(long, default_value = ".")]
    project: PathBuf,
    #[arg(long)]
    source_dir: Option<PathBuf>,
    #[arg(long)]
    test_dir: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
enum Command {
    Check,
    Fix {
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project = std::fs::canonicalize(&cli.project)?;
    match cli.command {
        Command::Check => {
            let diagnostics = qubit_infra_style::check_project(
                &project,
                cli.source_dir.as_deref(),
                cli.test_dir.as_deref(),
            )?;
            qubit_infra_style::print_diagnostics(&diagnostics, cli.format == Format::Json)?;
            if diagnostics.is_empty() {
                Ok(())
            } else {
                std::process::exit(1)
            }
        }
        Command::Fix { dry_run } => qubit_infra_style::fix_project(
            &project,
            cli.source_dir.as_deref(),
            cli.test_dir.as_deref(),
            dry_run,
        ),
    }
}
