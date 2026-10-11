use nix::libc;
use rtnetlink::{Handle, LinkBridge, LinkUnspec, LinkVeth, RouteMessageBuilder};
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};

use futures_lite::stream::StreamExt;

const JAILER_BRIDGE_NAME: &'static str = "jailer0";

async fn get_link_index(handle: &Handle, name: &str) -> Result<u32, Box<dyn std::error::Error>> {
    eprintln!("get_link_index: looking for {name}");

    eprintln!("get_link_index: before creating link query");
    let mut links = handle.link().get().match_name(name.to_string()).execute();

    eprintln!("get_link_index: before try_next");
    if let Some(msg) = links.try_next().await? {
        eprintln!("get_link_index: found index {}", msg.header.index);
        Ok(msg.header.index)
    } else {
        Err(format!("Interface '{name}' not found").into())
    }
}
pub async fn create_veth(
    handle: &Handle,
    host_side: &str,
    peer_side: &str,
) -> Result<(), Box<dyn Error>> {
    handle
        .link()
        .add(LinkVeth::new(host_side, peer_side).build())
        .execute()
        .await?;

    println!("veth created {} {}", host_side, peer_side);
    Ok(())
}

pub async fn up_veth(handle: &Handle, interface_name: &str) -> Result<(), Box<dyn Error>> {
    if let Ok(host_idx) = get_link_index(&handle, interface_name).await {
        handle
            .link()
            .set(LinkUnspec::new_with_index(host_idx).up().build())
            .execute()
            .await?;
    }
    println!("veth up {}", interface_name);
    Ok(())
}

#[derive(Clone)]
pub struct ContainerNetwork {
    cid: String,
}

impl ContainerNetwork {
    pub fn new(cid: String) -> Self {
        return Self { cid };
    }

    fn get_container_name(&self) -> String {
        format!("jc{}", &self.cid)
    }

    fn get_host_name(&self) -> String {
        format!("jh{}", &self.cid)
    }

    async fn move_link_to_namespace(
        &self,
        handle: &Handle,
        interface_name: &str,
        child_host_pid: u32,
    ) -> Result<(), Box<dyn Error>> {
        let interface_idx = get_link_index(handle, interface_name).await?;

        handle
            .link()
            .set(
                LinkUnspec::new_with_index(interface_idx)
                    .setns_by_pid(child_host_pid)
                    .build(),
            )
            .execute()
            .await?;

        println!(
            "moved veth to namespace {} {}",
            interface_name, child_host_pid
        );
        Ok(())
    }

    pub async fn link_container_wtih_bridge(
        &self,
        handle: &Handle,
        child_host_pid: u32,
    ) -> Result<(), Box<dyn Error>> {
        create_veth(handle, &self.get_host_name(), &self.get_container_name()).await?;

        self.move_link_to_namespace(handle, &self.get_container_name(), child_host_pid)
            .await?;

        println!("linked container with bridge {}", child_host_pid);
        Ok(())
    }

    pub async fn up_network_host_link(&self, handle: &Handle) -> Result<(), Box<dyn Error>> {
        let bridge_idx = get_link_index(handle, JAILER_BRIDGE_NAME).await?;
        let veth_idx = get_link_index(handle, &self.get_host_name()).await?;

        // Set the controller (master) property of the veth interface
        handle
            .link()
            .set(
                LinkUnspec::new_with_index(veth_idx)
                    .controller(bridge_idx) // Equivalent to 'master jailer0'
                    .build(),
            )
            .execute()
            .await?;

        up_veth(handle, &self.get_host_name()).await?;

        println!("host link up");
        Ok(())
    }

    pub async fn up_contianer_link(&self, handle: &Handle) -> Result<(), Box<dyn Error>> {
        eprintln!("net: before get_link_index");
        let container_veth_idx = get_link_index(handle, &self.get_container_name()).await?;
        eprintln!("net: after get_link_index: {container_veth_idx}");

        eprintln!("net: before eth0 rename");
        handle
            .link()
            .set(
                LinkUnspec::new_with_index(container_veth_idx)
                    .name("eth0") // Equivalent to 'set veth-container name eth0'
                    .build(),
            )
            .execute()
            .await?;

        println!(
            "net: after renamed container_veth_idx {} to eth0",
            container_veth_idx
        );

        eprintln!("net: before lo up");
        handle
            .link()
            .set(
                LinkUnspec::new_with_name("lo")
                    .up() // Equivalent to 'set lo up'
                    .build(),
            )
            .execute()
            .await?;

        println!("net: container lo up");

        eprintln!("net: before eth0 up");
        handle
            .link()
            .set(
                LinkUnspec::new_with_name("eth0")
                    .up() // Equivalent to 'set eth0 up'
                    .build(),
            )
            .execute()
            .await?;

        println!("net: eth0 up");

        println!("net: before find container_eth0_idx");
        let container_eth0_idx = get_link_index(handle, "eth0").await?;
        println!("net: found container_eth0_idx {}", container_eth0_idx);

        //10.10.0.2/24 dev eth0
        // TODO update this to track the Ips
        handle
            .address()
            .add(container_eth0_idx, Ipv4Addr::new(10, 10, 0, 2).into(), 24)
            .execute()
            .await?;

        println!("assigned up to eth0 {}", container_eth0_idx);

        // ip route add default via 10.10.0.1
        let route = RouteMessageBuilder::<Ipv4Addr>::new()
            .destination_prefix(Ipv4Addr::UNSPECIFIED, 0)
            .gateway(Ipv4Addr::new(10, 10, 0, 1))
            .build();
        handle.route().add(route).execute().await?;

        println!("container link up");
        Ok(())
    }
}

pub struct DaemonBridge;

impl DaemonBridge {
    pub async fn create_if_not_exists(handle: &Handle) -> Result<(), Box<dyn std::error::Error>> {
        let br_idx = match handle
            .link()
            .add(LinkBridge::new(JAILER_BRIDGE_NAME).build())
            .execute()
            .await
        {
            Ok(()) => get_link_index(&handle, JAILER_BRIDGE_NAME).await?,
            Err(rtnetlink::Error::NetlinkError(err))
                if err.code.is_some_and(|code| code.get() == -libc::EEXIST) =>
            {
                // EEXIST: the bridge already exists.
                get_link_index(&handle, JAILER_BRIDGE_NAME).await?
            }
            Err(err) => return Err(err.into()),
        };

        println!("bridge index: {}", br_idx);

        // Bring the bridge up.
        handle
            .link()
            .set(LinkUnspec::new_with_index(br_idx).up().build())
            .execute()
            .await?;

        println!("beidge up: {}", br_idx);

        // Assign the gateway IP.
        // For full idempotency, check whether this address already exists first.
        match handle
            .address()
            .add(br_idx, Ipv4Addr::new(10, 10, 0, 1).into(), 24)
            .execute()
            .await
        {
            Ok(()) => {}
            Err(rtnetlink::Error::NetlinkError(err))
                if err.code.is_some_and(|code| code.get() == -libc::EEXIST) => {} // already configured
            Err(err) => return Err(err.into()),
        };
        println!("address assigned: {}", br_idx);
        Ok(())
    }
}
