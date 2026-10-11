use crate::{command::Commands, container, ipc};
use container::manager::ContainerManager;
use container::network::DaemonBridge;
use rtnetlink::new_connection;
use std::{
    io::{self, BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

pub async fn start() -> Result<(), Box<dyn std::error::Error>> {
    let server = ipc::IpcServer::new(ipc::IPC_SOCK_PATH)?;

    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);
    // create bridge
    DaemonBridge::create_if_not_exists(&handle).await?;

    let mut deamon = Daemon::new();

    for connection in server.accept_incoming() {
        let stream = connection?;

        if deamon.handle_connection(stream) {
            break;
        }
    }

    Ok(())
}

struct Daemon {
    container_manager: ContainerManager,
}

impl Daemon {
    pub fn new() -> Self {
        Self {
            container_manager: ContainerManager::new(),
        }
    }

    pub fn handle_connection(&mut self, mut stream: UnixStream) -> bool {
        let mut request = String::new();

        {
            let mut reader = BufReader::new(&stream);

            if reader.read_line(&mut request).is_err() {
                return false;
            }
        }

        let command = match Commands::try_from(request.trim()) {
            Ok(command) => command,
            Err(_) => {
                let _ = stream.write_all(b"unknown command\n");
                return false;
            }
        };

        let (response, should_stop) = self.handle_command(&command, &mut stream);

        let response = format!("{response}\n<END>\n");
        if let Err(error) = stream.write_all(response.as_bytes()) {
            eprintln!("Failed to write response: {error}");
        }

        should_stop
    }

    fn handle_command(&mut self, command: &Commands, _stream: &mut UnixStream) -> (String, bool) {
        match command {
            Commands::Ping => ("pong\n".to_string(), false),
            Commands::Status => ("runtime is running\n".to_string(), false),
            Commands::Shutdown => ("daemon stopping\n".to_string(), true),
            Commands::Create { image, .. } => {
                if self.container_manager.create(image.clone()).is_err() {
                    return ("container creation error\n".to_string(), false);
                }

                ("container created\n".to_string(), false)
            }
            Commands::Stop { name } => {
                if self.container_manager.stop(name.clone()).is_err() {
                    return ("container stoping error\n".to_string(), false);
                }
                ("container stopping\n".to_string(), false)
            }
            Commands::Pause { name } => {
                if self.container_manager.pause(name.clone()).is_err() {
                    return ("container pausing error\n".to_string(), false);
                }
                ("container paused\n".to_string(), false)
            }
            Commands::PS => match self.container_manager.ps() {
                Ok(resp) => (resp, false),
                Err(_) => ("container ps error\n".to_string(), false),
            },
            _ => ("unsupported command\n".to_string(), false),
        }
    }
}
