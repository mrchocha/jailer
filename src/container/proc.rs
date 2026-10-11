use cgroups_rs::fs::Cgroup;
use cgroups_rs::fs::cgroup_builder::CgroupBuilder;
use nix::mount::{MntFlags, MsFlags, mount, umount2};
use nix::sched::CloneFlags;
use nix::sched::clone;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use nix::unistd::chdir;
use nix::unistd::execv;
use std::ffi::CString;
use std::fmt;

use crate::container::cgroup::ContainerProcessCgroups;

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
}

impl ConntainerProcess {
    fn setup_rootfs(rootfs: &str) -> Result<(), Box<dyn std::error::Error>> {
        let root = CString::new(rootfs)?;

        // 1. Stop mount events propagating to the host.
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )?;

        // 2. Make rootfs a mount point.
        mount(
            Some(rootfs),
            rootfs,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )?;

        // 3. Create the directory for the old root.
        let oldroot = format!("{rootfs}/oldroot");
        std::fs::create_dir_all(&oldroot)?;

        let oldroot_c = CString::new(oldroot)?;

        // 4. Switch the process root.
        let result = unsafe {
            nix::libc::syscall(nix::libc::SYS_pivot_root, root.as_ptr(), oldroot_c.as_ptr())
        };

        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }

        // 5. Work from the new root.
        chdir("/")?;

        // 6. Detach the old root.
        umount2("/oldroot", MntFlags::MNT_DETACH)?;
        std::fs::remove_dir("/oldroot")?;

        // 7. Mount proc for process information.
        std::fs::create_dir_all("/proc")?;
        mount(
            Some("proc"),
            "/proc",
            Some("proc"),
            MsFlags::empty(),
            None::<&str>,
        )?;

        Ok(())
    }

    pub fn new(name: String) -> Result<Self, Box<dyn std::error::Error>> {
        // clone() needs a stack for the child.
        let mut stack = vec![0u8; 1024 * 1024];

        let flags = CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS;

        let child_name = name.clone();

        let child = unsafe {
            clone(
                Box::new(move || -> isize {
                    println!("Container: {child_name}");
                    println!("PID inside namespace: {}", nix::unistd::getpid());

                    let shell = CString::new("/bin/bash").unwrap();
                    let arg0 = CString::new("bash").unwrap();
                    let option = CString::new("-c").unwrap();
                    let command = CString::new("sleep 60").unwrap();
                    let args = [arg0, option, command];

                    match execv(&shell, &args) {
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
        let child_process = Self { pid: child, cg };

        Ok(child_process)
    }

    pub fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.cg.cleanup_cgroup()?;

        Ok(())
    }
}
