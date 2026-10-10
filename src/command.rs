use std::fmt;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "container-runtime",
    version,
    about = "A simple container runtime"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the daemon
    Daemon,

    /// Check whether the daemon is responding
    Ping,

    /// Show daemon status
    Status,

    /// Shutdown the deamon
    Shutdown,

    /// Stop a container
    Stop { container_id: String },

    /// Pause a container
    Pause { container_id: String },

    /// list running container
    PS,

    /// Create a container from an image
    Create {
        image: String,

        #[arg(long)]
        name: Option<String>,

        #[arg(long)]
        memory: Option<u64>,
    },
}

impl fmt::Display for Commands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Commands::Ping => write!(f, "ping"),
            Commands::Status => write!(f, "status"),
            Commands::Stop { container_id } => {
                write!(f, "stop {container_id}")
            }
            Commands::Pause { container_id } => {
                write!(f, "pause {container_id}")
            }
            Commands::Create { image, .. } => {
                write!(f, "create {image}")
            }
            Commands::Daemon => write!(f, "daemon"),
            Commands::Shutdown => write!(f, "shutdown"),
            Commands::PS => write!(f, "ps"),
        }
    }
}

impl TryFrom<&str> for Commands {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut parts = value.split_whitespace();
        let command = parts.next().unwrap_or("").to_ascii_uppercase();

        match command.as_str() {
            "PING" => Ok(Commands::Ping),
            "STATUS" => Ok(Commands::Status),
            "SHUTDOWN" => Ok(Commands::Shutdown),

            "STOP" => {
                let container_id = parts.next().ok_or("Missing container ID")?;

                Ok(Commands::Stop {
                    container_id: container_id.to_string(),
                })
            }

            "PAUSE" => {
                let container_id = parts.next().ok_or("Missing container ID")?;

                Ok(Commands::Pause {
                    container_id: container_id.to_string(),
                })
            }

            "CREATE" => {
                let image = parts.next().ok_or("Missing image name")?;

                Ok(Commands::Create {
                    image: image.to_string(),
                    name: None,
                    memory: None,
                })
            }

            "DAEMON" => Ok(Commands::Daemon),
            "PS" => Ok(Commands::PS),

            _ => Err(format!("Unknown command: {command}")),
        }
    }
}
