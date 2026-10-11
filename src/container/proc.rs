use crate::container::cgroup::ContainerProcessCgroups;
use crate::container::fs::ContainerProcessFileSystem;
use crate::container::network::ContainerNetwork;
use nix::sched::CloneFlags;
use nix::sched::clone;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use nix::unistd::execve;
use rtnetlink::new_connection;
use std::ffi::CString;
use std::fmt;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;

pub enum ConntainerProcessStatus {
    Running,
    Paused,
    Stopped,
}

impl fmt::Display for ConntainerProcessStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConntainerProcessStatus::Running => write!(f, "RUNNING"),
            ConntainerProcessStatus::Stopped => write!(f, "STOPED"),
            ConntainerProcessStatus::Paused => write!(f, "PAUSE"),
        }
    }
}

pub struct ConntainerProcess {
    pub pid: Pid,
    pub cg: ContainerProcessCgroups,
    pub fs: ContainerProcessFileSystem,
    pub net: ContainerNetwork,
}

impl ConntainerProcess {
    pub async fn new(cid: String) -> Result<Self, Box<dyn std::error::Error>> {
        // clone() needs a stack for the child.
        let mut stack = vec![0u8; 1024 * 1024];

        let flags = CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS;

        let (connection, handle, _) = new_connection()?;
        tokio::spawn(connection);

        let child_cid = cid.clone();

        let (mut parent_socket, mut child_socket) = UnixStream::pair()?;

        let child = unsafe {
            clone(
                Box::new(move || -> isize {
                    println!("PID inside namespace: {}", nix::unistd::getpid());
                    let mut signal = [0u8; 1];

                    if child_socket.read_exact(&mut signal).is_err() || signal[0] != 1 {
                        return 1;
                    }

                    println!("Settingup child process");

                    let path = CString::new("PATH=/usr/bin:/bin:/usr/sbin:/sbin").unwrap();
                    let home = CString::new("HOME=/root").unwrap();
                    let environment = [path, home];
                    let child = CString::new(child_cid.clone()).unwrap();

                    let jailer_binary =
                        CString::new("/home/rahul/jailer/target/debug/jailer").unwrap();

                    match execve(
                        &jailer_binary,
                        &[
                            jailer_binary.clone(),
                            CString::new("container-init").unwrap(),
                            child,
                            // Additional setup arguments...
                        ],
                        &environment,
                    ) {
                        Ok(_) => 0,
                        Err(err) => {
                            eprintln!("Failed to start bash: {err}");
                            1
                        }
                    }
                }),
                &mut stack,
                flags,
                Some(Signal::SIGCHLD as i32),
            )?
        };

        let net = ContainerNetwork::new(cid.clone());
        let fs = ContainerProcessFileSystem::new(cid.clone());

        net.link_container_wtih_bridge(&handle, child.as_raw() as u32)
            .await?;
        net.up_network_host_link(&handle).await?;
        parent_socket.write_all(&[1])?;

        // This runs in the parent process.
        println!("Parent PID: {}", nix::unistd::getpid());
        println!("Container child PID: {child}");

        let cg = ContainerProcessCgroups::new(child)?;
        let child_process = Self {
            pid: child,
            cg,
            fs,
            net,
        };

        Ok(child_process)
    }

    pub fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.cg.cleanup_cgroup()?;
        self.fs.cleanup()?;

        Ok(())
    }
}
