# Jailer

A minimal container runtime written in Rust. It isolates processes using Linux namespaces, enforces resource limits with cgroups v2, and sets up basic networking.

## Features

- **Namespace isolation**: PID, mount (mnt), network (net), and UTS namespaces
- **Resource limits**: CPU (750 millicores, 100ms period) and memory (512MB) via cgroups
- **Rootfs isolation**: Creates a minimal root filesystem, copies required binaries with their dependencies, and uses `pivot_root` for isolation
- **Networking**: Creates a bridge (`jailer0`), sets up veth pairs between host and container, assigns IPs, and configures routing
- **Daemon-based**: Runs as a daemon that manages containers and communicates with the CLI over a Unix socket (`/tmp/myruntime.sock`)

## Building

```bash
cargo build
```

## Usage

Start the daemon:

```bash
./target/debug/jailer daemon
```

Create a container:

```bash
./target/debug/jailer create <image>
```

List running containers:

```bash
./target/debug/jailer ps
```

Pause/Stop a container:

```bash
./target/debug/jailer pause <name>
./target/debug/jailer stop <name>
```

Check daemon status/ping/shutdown:

```bash
./target/debug/jailer status
./target/debug/jailer ping
./target/debug/jailer shutdown
```

## Notes

- The runtime copies a small set of binaries (`bash`, `ls`, `pwd`, `ping`, `cat`, `ip`) and their shared libraries into the container rootfs
- The container currently runs a hardcoded `ping -c 4 10.10.0.1` on startup
- Requires Linux with cgroups v2, root privileges, and access to `/sys/fs/cgroup`