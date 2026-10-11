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
        Commands::Daemon => {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(daemon::start())?;
        }
        Commands::Shutdown => cli::shutdown()?,
        Commands::Ping => cli::ping()?,
        Commands::Status => cli::status()?,
        Commands::PS => cli::ps()?,
        Commands::ContainerInit { cid } => {
            cli::containerinit(&cid)?;
        }
        Commands::Stop { name } => {
            cli::stop(&name)?;
        }
        Commands::Pause { name } => {
            cli::pause(&name)?;
        }
        Commands::Create { image, .. } => cli::create(&image)?,
    }

    Ok(())
}
