use crate::container::{fs::ContainerProcessFileSystem, network::ContainerNetwork};
use nix::unistd::execve;
use rtnetlink::new_connection;
use std::error::Error;
use std::ffi::CString;

pub struct ContainerInit {
    cid: String,
    net: ContainerNetwork,
    fs: ContainerProcessFileSystem,
}

impl ContainerInit {
    pub fn new(cid: String) -> Self {
        let net = ContainerNetwork::new(cid.clone());
        let fs = ContainerProcessFileSystem::new(cid.clone());

        Self { cid, net, fs }
    }

    pub fn run_container_init(&self) -> Result<(), Box<dyn Error>> {
        // Configure networking using a runtime initialized by this process.
        {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;

            runtime.block_on(async {
                let (connection, handle, _) = new_connection()?;

                // Drive the netlink connection while configuring the interface.
                let connection_task = tokio::spawn(connection);

                let result = self.net.up_contianer_link(&handle).await;

                connection_task.abort();
                result
            })?;
        }

        // Set up the container's root filesystem.
        self.fs.isolate()?;

        // Execute the container command with an explicit environment.
        let shell = CString::new("/usr/bin/bash")?;
        let arg0 = CString::new("bash")?;
        let option = CString::new("-c")?;
        let command = CString::new("ping -c 4 10.10.0.1")?;

        let path = CString::new("PATH=/usr/bin:/bin:/usr/sbin:/sbin")?;
        let home = CString::new("HOME=/root")?;

        let args = [arg0, option, command];
        let env = [path, home];

        // On success, execve replaces this process and never returns.
        execve(&shell, &args, &env)?;

        Ok(())
    }
}
