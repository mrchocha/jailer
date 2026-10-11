use nix::mount::{MntFlags, MsFlags, mount, umount2};
use nix::unistd::chdir;
use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ContainerProcessFileSystem {
    cid: String,
}

impl ContainerProcessFileSystem {
    pub fn new(cid: String) -> Self {
        Self { cid }
    }

    fn run_command(
        &self,
        program: &str,
        args: &[&str],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let output = Command::new(program).args(args).output()?;

        if !output.status.success() {
            return Err(format!(
                "{} failed: {}",
                program,
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    fn find_binary(&self, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let output = self.run_command("which", &[name])?;
        Ok(PathBuf::from(output.trim()))
    }

    fn copy_file(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(source, destination)?;
        Ok(())
    }

    fn copy_binary(&self, binary: &Path, rootfs: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let name = binary.file_name().ok_or("Invalid binary path")?;

        let destination = rootfs.join("usr/bin").join(name);

        println!("Copying binary: {}", binary.display());
        self.copy_file(binary, &destination)?;

        // Discover shared library dependencies.
        let output = Command::new("ldd").arg(binary).output()?;

        if !output.status.success() {
            return Err(format!(
                "ldd failed for {}: {}",
                binary.display(),
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        let output = String::from_utf8(output.stdout)?;

        for line in output.lines() {
            if let Some(library) = self.parse_library_path(line) {
                let source = Path::new(&library);

                // Preserve the absolute library path inside rootfs.
                let destination = rootfs.join(source.strip_prefix("/").unwrap_or(source));

                println!("Copying library: {}", source.display());

                // Resolve symlinks and copy the actual file.
                let resolved = fs::canonicalize(source)?;
                self.copy_file(&resolved, &destination)?;
            }
        }

        Ok(())
    }

    fn parse_library_path(&self, line: &str) -> Option<String> {
        // Example: libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (...)
        if let Some((_, right)) = line.split_once("=>") {
            let path = right.split_whitespace().next()?;

            if path.starts_with('/') && Path::new(path).exists() {
                return Some(path.to_string());
            }

            return None;
        }

        // Example: /lib64/ld-linux-x86-64.so.2 (...)
        let first = line.split_whitespace().next()?;

        if first.starts_with('/') && Path::new(first).exists() {
            return Some(first.to_string());
        }

        None
    }

    fn get_root_fs_path(&self) -> String {
        "/tmp/jailer/".to_string() + self.cid.as_str()
    }

    fn create_process_root_fs_dir(&self) -> Result<(), Box<dyn std::error::Error>> {
        let rootfs = self.get_root_fs_path();
        std::fs::create_dir_all(rootfs.clone())?;
        Ok(())
    }

    // Stop mount events propagating to the host.
    fn setup_mount_namespace(&self) -> Result<(), Box<dyn std::error::Error>> {
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )?;
        Ok(())
    }

    fn setup_rootfs(&self) -> Result<(), Box<dyn std::error::Error>> {
        let rootfs = self.get_root_fs_path();

        //  Make rootfs a mount point.
        mount(
            Some(rootfs.as_str()),
            rootfs.as_str(),
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )?;

        //  Create the directory for the old root.
        let oldroot = format!("{rootfs}/oldroot");
        std::fs::create_dir_all(&oldroot)?;

        let oldroot_c = CString::new(oldroot)?;
        let new_root = CString::new(rootfs)?;

        // Switch the process root.
        let result = unsafe {
            nix::libc::syscall(
                nix::libc::SYS_pivot_root,
                new_root.as_ptr(),
                oldroot_c.as_ptr(),
            )
        };

        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }

        //  Work from the new root.
        chdir("/")?;

        //  Detach the old root.
        umount2("/oldroot", MntFlags::MNT_DETACH)?;
        std::fs::remove_dir("/oldroot")?;

        // Mount proc for process information.
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

    pub fn isolate(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.create_process_root_fs_dir()?;

        let rootfs = PathBuf::from(self.get_root_fs_path());
        fs::create_dir_all(rootfs.join("usr/bin"))?;

        for name in ["bash", "ls", "pwd", "ping"] {
            let binary = self.find_binary(name)?;
            self.copy_binary(&binary, &rootfs)?;
        }

        self.setup_mount_namespace()?;
        self.setup_rootfs()?;
        Ok(())
    }
}
