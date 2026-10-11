use nix::libc;
use rtnetlink::{Handle, LinkBridge, LinkUnspec, LinkVeth, RouteMessageBuilder};
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};

use futures_lite::stream::StreamExt;

const JAILER_BRIDGE_NAME: &'static str = "jailer0";
const JAILER_HOST_VETH: &'static str = "jailer-veth-host";

async fn get_link_index(handle: &rtnetlink::Handle, name: &str) -> Result<u32, Box<dyn Error>> {
    let mut links = handle.link().get().match_name(name.to_string()).execute();
    if let Some(msg) = links.try_next().await? {
        Ok(msg.header.index)
    } else {
        Err(format!("Interface '{}' not found", name).into())
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
    Ok(())
}

pub struct ContainerNetwork {
    cid: String,
}

impl ContainerNetwork {
    pub fn new(cid: String) -> Self {
        return Self { cid };
    }

    fn get_container_name(&self) -> String {
        format!("jc{}", &self.cid[..8])
    }

    fn get_host_name(&self) -> String {
        format!("jh{}", &self.cid[..8])
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

        Ok(())
    }

    pub async fn link_container_wtih_bridge(
        &self,
        handle: &Handle,
        child_host_pid: u32,
    ) -> Result<(), Box<dyn Error>> {
        create_veth(handle, &self.get_host_name(), &self.get_container_name()).await?;

        self.move_link_to_namespace(handle, &self.get_container_name(), child_host_pid);

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

        Ok(())
    }

    pub async fn up_contianer_link(&self, handle: &Handle) -> Result<(), Box<dyn Error>> {
        let container_veth_idx = get_link_index(handle, &self.get_container_name()).await?;

        handle
            .link()
            .set(
                LinkUnspec::new_with_index(container_veth_idx)
                    .name("eth0") // Equivalent to 'set veth-container name eth0'
                    .build(),
            )
            .execute()
            .await?;

        handle
            .link()
            .set(
                LinkUnspec::new_with_name("lo")
                    .up() // Equivalent to 'set lo up'
                    .build(),
            )
            .execute()
            .await?;

        handle
            .link()
            .set(
                LinkUnspec::new_with_name("eth0")
                    .up() // Equivalent to 'set eth0 up'
                    .build(),
            )
            .execute()
            .await?;

        let container_eth0_idx = get_link_index(handle, "eth0").await?;

        //10.10.0.2/24 dev eth0
        // TODO update this to track the Ips
        handle
            .address()
            .add(container_eth0_idx, Ipv4Addr::new(10, 10, 0, 2).into(), 24)
            .execute()
            .await?;

        // ip route add default via 10.10.0.1
        let route = RouteMessageBuilder::<Ipv4Addr>::new()
            .destination_prefix(Ipv4Addr::UNSPECIFIED, 0)
            .gateway(Ipv4Addr::new(10, 10, 0, 1))
            .build();
        handle.route().add(route).execute().await?;

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
                if err.code.is_some_and(|code| code.get() == libc::EEXIST) =>
            {
                // EEXIST: the bridge already exists.
                get_link_index(&handle, JAILER_BRIDGE_NAME).await?
            }
            Err(err) => return Err(err.into()),
        };

        // Bring the bridge up.
        handle
            .link()
            .set(LinkUnspec::new_with_index(br_idx).up().build())
            .execute()
            .await?;

        // Assign the gateway IP.
        // For full idempotency, check whether this address already exists first.
        handle
            .address()
            .add(br_idx, Ipv4Addr::new(10, 10, 0, 1).into(), 24)
            .execute()
            .await?;

        Ok(())
    }
}
