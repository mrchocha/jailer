use cgroups_rs::fs::{Cgroup, cgroup_builder::CgroupBuilder};
use nix::unistd::Pid;

pub struct ContainerProcessCgroups {
    cg: Cgroup,
}

impl ContainerProcessCgroups {
    fn millicores_to_quota(millicores: u64, period_us: u64) -> u64 {
        // Equation: (millicores / 1000) * period
        (millicores * period_us) / 1000
    }

    pub fn new(pid: Pid) -> Result<Self, Box<dyn std::error::Error>> {
        let hier = cgroups_rs::fs::hierarchies::auto();

        let max_memory_bytes: i64 = 512 * 1024 * 1024;
        let requested_millicores = 750;
        let cfs_period_us = 100_000;
        let cfs_quota_us = Self::millicores_to_quota(requested_millicores, cfs_period_us);

        let cg: Cgroup = CgroupBuilder::new(&pid.to_string())
            .cpu()
            .period(cfs_period_us)
            .quota(cfs_quota_us as i64)
            .done()
            .memory()
            .memory_hard_limit(max_memory_bytes)
            .done()
            .build(hier)
            .map_err(|e| {
                eprintln!("cgroup: build failed: {e:?}");
                e
            })?;

        let child_pid = pid.as_raw();

        let procs_path = std::path::Path::new("/sys/fs/cgroup")
            .join(cg.path())
            .join("cgroup.procs");

        std::fs::write(&procs_path, child_pid.to_string())?;

        Ok(Self { cg })
    }

    pub fn cleanup_cgroup(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.cg.delete()?;

        Ok(())
    }
}
