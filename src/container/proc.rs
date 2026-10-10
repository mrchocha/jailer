use nix::libc::sleep;
use nix::sched::CloneFlags;
use nix::sched::clone;
use nix::sys::signal::Signal;
use nix::unistd::Pid;

pub struct ConntainerProcess {
    pub pid: Pid,
    pub name: String,
}

impl ConntainerProcess {
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

                    loop {
                        println!("this is child process");
                        sleep(2);
                    }

                    0
                }),
                &mut stack,
                flags,
                Some(Signal::SIGCHLD as i32),
            )?
        };

        // This runs in the parent process.
        println!("Parent PID: {}", nix::unistd::getpid());
        println!("Container child PID: {child}");

        Ok(Self {
            pid: child,
            name: container_id,
        })
    }
}
