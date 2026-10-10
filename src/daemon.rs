use std::{
    io::{self, BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

use crate::{command::Commands, container, ipc};
use container::manager::ContainerManager;

pub fn start() -> io::Result<()> {
    let server = ipc::IpcServer::new(ipc::IPC_SOCK_PATH)?;
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

        if let Err(error) = stream.write_all(response.as_bytes()) {
            eprintln!("Failed to write response: {error}");
        }

        should_stop
    }

    fn handle_command(
        &mut self,
        command: &Commands,
        _stream: &mut UnixStream,
    ) -> (&'static str, bool) {
        match command {
            Commands::Ping => ("pong\n", false),
            Commands::Status => ("runtime is running\n", false),
            Commands::Shutdown => ("daemon stopping\n", true),
            Commands::Stop { .. } => ("container stopping\n", false),
            Commands::Create { image, .. } => {
                if self.container_manager.create(image.clone()).is_err() {
                    return ("container creation error\n", false);
                }

                ("container created\n", false)
            }

            Commands::PS => {
                if self.container_manager.ps().is_err() {
                    return ("container ps error\n", false);
                }

                ("\n", false)
            }
            _ => ("unsupported command\n", false),
        }
    }
}
