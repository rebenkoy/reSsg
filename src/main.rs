mod config;
mod build;
mod cli_parser;
mod server;

use clap::{Parser, Subcommand};
use crate::build::build;
use crate::server::serve;
use crate::config::reSsgConfig;

/// Simple program to greet a person
#[derive(Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    command: Command
}

#[derive(Subcommand, Debug)]
enum Command {
    Build,
    Serve,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();
    std::env::var("RESSG_ROOT").map_or(Ok(()), |dir| {
        std::env::set_current_dir(dir)
    })?;

    let config_file = std::env::current_dir()?
        .join("config.toml");

    let config: reSsgConfig = toml::from_slice(&std::fs::read(config_file)?)?;
    match args.command {
        Command::Build => {
            build(&config.build, &mut rsfs::disk::FS{})?;
        }
        Command::Serve => {
            serve(&config)?;
        }
    }
    Ok(())
}