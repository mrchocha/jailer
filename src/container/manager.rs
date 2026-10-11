use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;

use crate::container::proc::{ConntainerProcess, ConntainerProcessStatus};
use nix::sys::signal::{Signal, kill};

struct ConntainerProcessInfo {
    process: ConntainerProcess,
    status: ConntainerProcessStatus,
}

pub struct ContainerManager {
    id_map: Arc<RwLock<HashMap<String, ConntainerProcessInfo>>>,
}

impl ContainerManager {
    pub fn new() -> Self {
        Self {
            id_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn create(&mut self, container_id: String) -> Result<(), Box<dyn std::error::Error>> {
        let id_map_clone = Arc::clone(&self.id_map);

        let handler = thread::spawn(move || match ConntainerProcess::new(container_id.clone()) {
            Ok(process) => {
                if let Ok(mut id_map) = id_map_clone.write() {
                    id_map.insert(
                        container_id.clone(),
                        ConntainerProcessInfo {
                            process,
                            status: ConntainerProcessStatus::Running,
                        },
                    );
                }
            }
            Err(err) => eprintln!("Container creation failed: {err:?}"),
        });

        handler.join().expect("Error on join");

        Ok(())
    }

    pub fn ps(&self) -> Result<String, Box<dyn std::error::Error + '_>> {
        let id_map = self.id_map.read()?;
        let mut output = String::new();

        output.push_str(&format!(
            "{:<15} {:<8} {:<20} {:<10}\n",
            "CONTAINER ID", "PID", "NAME", "STATUS"
        ));

        for (pid, container) in id_map.iter() {
            output.push_str(&format!(
                "{:<15} {:<8} {:<20} {:<10}\n",
                pid, container.process.pid, container.process.name, container.status
            ));
        }

        Ok(output)
    }

    pub fn stop(&self, container_id: String) -> Result<(), Box<dyn std::error::Error + '_>> {
        let pid = {
            let mut id_map = self.id_map.write()?;

            let container = id_map.get_mut(&container_id).ok_or("Container not found")?;

            container.status = ConntainerProcessStatus::Stopped;
            println!("Killing {}", container.process.pid);
            kill(container.process.pid, Signal::SIGKILL)?;

            container.process.pid
        };

        nix::sys::wait::waitpid(pid, None)?;

        let mut id_map = self.id_map.write()?;
        id_map.remove(&container_id);

        Ok(())
    }

    pub fn pause(&self, container_id: String) -> Result<(), Box<dyn std::error::Error + '_>> {
        let mut id_map = self.id_map.write()?;
        if let Some(container) = id_map.get_mut(&container_id) {
            kill(container.process.pid, Signal::SIGSTOP)?;
            container.status = ConntainerProcessStatus::Paused
        }
        Ok(())
    }
}
