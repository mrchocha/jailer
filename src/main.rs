use clap::Parser;
use command::Commands;
mod cli;
mod command;
mod container;
mod daemon;
mod ipc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = command::Cli::try_parse()?;

    match cli.command {
        Commands::Daemon => daemon::start()?,
        Commands::Shutdown => cli::shutdown()?,
        Commands::Ping => cli::ping()?,
        Commands::Status => cli::status()?,
        Commands::PS => cli::ps()?,
        Commands::Stop { container_id } => {
            cli::stop(&container_id)?;
        }
        Commands::Create {
            image,
            name,
            memory,
        } => cli::create(&image)?,
    }

    Ok(())
}
