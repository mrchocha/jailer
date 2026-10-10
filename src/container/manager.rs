use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;

use nix::unistd::Pid;

use crate::container::proc::ConntainerProcess;

pub struct ContainerManager {
    id_map: Arc<RwLock<HashMap<Pid, ConntainerProcess>>>,
}

impl ContainerManager {
    pub fn new() -> Self {
        Self {
            id_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn create(&mut self, container_id: String) -> Result<(), Box<dyn std::error::Error>> {
        let id_map_clone = Arc::clone(&self.id_map);

        let handler = thread::spawn(move || match ConntainerProcess::new(container_id) {
            Ok(process) => {
                if let Ok(mut id_map) = id_map_clone.write() {
                    id_map.insert(process.pid, process);
                }
            }
            Err(err) => eprintln!("Container creation failed: {err:?}"),
        });

        handler.join().expect("Error on join");

        Ok(())
    }

    pub fn ps(&self) -> Result<String, Box<dyn std::error::Error + '_>> {
        let mut string = String::new();

        let id_map = self.id_map.read()?;
        for (pid, container) in id_map.iter() {
            string += format!("{:?} {:?}", pid, container.name).as_str();
        }

        return Ok(string);
    }
}
