use std::io;

use crate::{
    command::Commands,
    ipc::{IPC_SOCK_PATH, IpcClient},
};

pub fn ping() -> io::Result<()> {
    let ipc = IpcClient::new(IPC_SOCK_PATH);
    let resp = ipc.send_request(&Commands::Ping)?;
    println!("{:?}", resp);
    Ok(())
}

pub fn shutdown() -> io::Result<()> {
    let ipc = IpcClient::new(IPC_SOCK_PATH);
    let resp = ipc.send_request(&Commands::Shutdown)?;
    println!("{:?}", resp);
    Ok(())
}

pub fn status() -> io::Result<()> {
    let ipc = IpcClient::new(IPC_SOCK_PATH);
    let resp = ipc.send_request(&Commands::Status)?;
    println!("{:?}", resp);
    Ok(())
}

pub fn ps() -> io::Result<()> {
    let ipc = IpcClient::new(IPC_SOCK_PATH);
    let resp = ipc.send_request(&Commands::PS)?;
    println!("{:?}", resp);
    Ok(())
}

pub fn stop(container_id: &str) -> io::Result<()> {
    let ipc = IpcClient::new(IPC_SOCK_PATH);
    let resp = ipc.send_request(&Commands::Stop {
        container_id: container_id.to_owned(),
    })?;
    println!("{:?}", resp);
    Ok(())
}

pub fn create(image: &str) -> io::Result<()> {
    let ipc = IpcClient::new(IPC_SOCK_PATH);
    let resp = ipc.send_request(&Commands::Create {
        image: image.to_owned(),
        name: None,
        memory: None,
    })?;
    println!("{:?}", resp);
    Ok(())
}
