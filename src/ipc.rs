use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;

use crate::command::Commands;

pub const IPC_SOCK_PATH: &str = "/tmp/myruntime.sock";

pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    pub fn new(path: impl Into<String>) -> io::Result<Self> {
        let unix_sock_path = path.into();
        let socket = Path::new(&unix_sock_path);

        if socket.exists() {
            match UnixStream::connect(&unix_sock_path) {
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        "daemon is already running",
                    ));
                }
                Err(_) => fs::remove_file(socket)?,
            }
        }

        let listener = UnixListener::bind(&unix_sock_path)?;

        Ok(Self { listener })
    }

    pub fn accept_incoming(&self) -> impl Iterator<Item = io::Result<UnixStream>> + '_ {
        self.listener.incoming()
    }
}

pub struct IpcClient {
    unix_sock_path: PathBuf,
}

impl IpcClient {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            unix_sock_path: path.into(),
        }
    }

    pub fn send_request(&self, request: &Commands) -> io::Result<String> {
        let mut stream = UnixStream::connect(&self.unix_sock_path)?;

        writeln!(stream, "{request}")?;

        let mut response = String::new();
        BufReader::new(stream).read_line(&mut response)?;
        Ok(response)
    }
}
