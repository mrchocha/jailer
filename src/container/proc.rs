use nix::sched::CloneFlags;
use nix::sched::clone;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use nix::unistd::execve;
use std::ffi::CString;
use std::fmt;

use crate::container::cgroup::ContainerProcessCgroups;
use crate::container::fs::ContainerProcessFileSystem;

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
}

impl ConntainerProcess {
    pub fn new(cid: String) -> Result<Self, Box<dyn std::error::Error>> {
        // clone() needs a stack for the child.
        let mut stack = vec![0u8; 1024 * 1024];

        let flags = CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS;

        let child_cid = cid.clone();
        let fs = ContainerProcessFileSystem::new(child_cid);

        let child_fs = fs.clone();
        let child = unsafe {
            clone(
                Box::new(move || -> isize {
                    println!("PID inside namespace: {}", nix::unistd::getpid());
                    child_fs
                        .isolate()
                        .expect("error while creating isloated fs");

                    let shell = CString::new("/usr/bin/bash").unwrap();
                    let arg0 = CString::new("bash").unwrap();
                    let option = CString::new("-c").unwrap();
                    let command = CString::new("ip addr").unwrap();

                    let path = CString::new("PATH=/usr/bin:/bin:/usr/sbin:/sbin").unwrap();
                    let home = CString::new("HOME=/root").unwrap();
                    let env = [path, home];

                    let args = [arg0, option, command];

                    match execve(&shell, &args, &env) {
                        Ok(_) => unreachable!(),
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

        // This runs in the parent process.
        println!("Parent PID: {}", nix::unistd::getpid());
        println!("Container child PID: {child}");

        let cg = ContainerProcessCgroups::new(child)?;
        let child_process = Self { pid: child, cg, fs };

        Ok(child_process)
    }

    pub fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.cg.cleanup_cgroup()?;
        self.fs.cleanup()?;

        Ok(())
    }
}
