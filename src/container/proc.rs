use cgroups_rs::fs::Cgroup;
use cgroups_rs::fs::cgroup_builder::CgroupBuilder;
use cgroups_rs::*;
use nix::mount::{MntFlags, MsFlags, mount, umount2};
use nix::sched::CloneFlags;
use nix::sched::clone;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use nix::unistd::chdir;
use nix::unistd::execv;
use std::ffi::CString;
use std::fmt;
use std::path::PathBuf;

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
    pub name: String,
}

impl ConntainerProcess {
    fn millicores_to_quota(&self, millicores: u64, period_us: u64) -> u64 {
        // Equation: (millicores / 1000) * period
        (millicores * period_us) / 1000
    }

    fn apply_resource_limits(&self) -> Result<(), Box<dyn std::error::Error>> {
        let hier = cgroups_rs::fs::hierarchies::auto();

        let max_memory_bytes: i64 = 512 * 1024 * 1024;
        let requested_millicores = 750;
        let cfs_period_us = 100_000;
        let cfs_quota_us = self.millicores_to_quota(requested_millicores, cfs_period_us);

        let cg: Cgroup = CgroupBuilder::new(self.name.as_str())
            .cpu()
            .period(cfs_period_us)
            .quota(cfs_quota_us as i64)
            .done()
            .memory()
            .memory_hard_limit(max_memory_bytes)
            .done()
            .build(hier)?;

        let chuld_pid = self.pid.as_raw() as u64;

        cg.add_task(CgroupPid::from(chuld_pid))?;
        Ok(())
    }

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

    pub fn new(container_id: String) -> Result<Self, Box<dyn std::error::Error>> {
        // clone() needs a stack for the child.
        let mut stack = vec![0u8; 1024 * 1024];

        let flags = CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS;

        let child_name = container_id.clone();

        let child = unsafe {
            clone(
                Box::new(move || -> isize {
                    println!("Container: {child_name}");
                    println!("PID inside namespace: {}", nix::unistd::getpid());

                    let shell = CString::new("/bin/bash").unwrap();
                    let arg0 = CString::new("bash").unwrap();
                    let option = CString::new("-c").unwrap();
                    let command = CString::new("ip link set lo up && ip addr show lo").unwrap();
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

        let child_process = Self {
            pid: child,
            name: container_id,
        };

        child_process.apply_resource_limits()?;
        Ok(child_process)
    }
}
