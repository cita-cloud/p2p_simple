use channel::unbounded;
use log::info;
use p2p_simple::{channel, P2P};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::str;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

// run example:
// RUST_LOG=info cargo run --example test false 0
// RUST_LOG=info cargo run --example test false 1

// RUST_LOG=info cargo run --example test true 0
// RUST_LOG=info cargo run --example test true 1
// mkfifo /tmp/fifo0
// mkfifo /tmp/fifo0
// bsd-nc -vlk 8338 0</tmp/fifo1 | bsd-nc localhost 1337 1>/tmp/fifo1
// bsd-nc -vlk 8337 0</tmp/fifo0 | bsd-nc localhost 1338 1>/tmp/fifo0
fn main() {
    env_logger::init();

    let path0 = Path::new(".")
        .join("examples")
        .join("0_privkey")
        .to_str()
        .unwrap()
        .to_string();
    let path1 = Path::new(".")
        .join("examples")
        .join("1_privkey")
        .to_str()
        .unwrap()
        .to_string();
    let addr_0 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1337);
    let addr_1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1338);

    let arg0 = std::env::args().nth(1).unwrap();
    let is_need_proxy = FromStr::from_str(&arg0).unwrap();

    let target_0;
    let target_1;
    if is_need_proxy {
        target_0 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8337);
        target_1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8338);
    } else {
        target_0 = addr_1;
        target_1 = addr_0;
    }
    if std::env::args().nth(2) == Some("0".to_string()) {
        info!("Starting node 0 ......");
        let (tx, rx) = unbounded();
        let s = P2P::new(
            path0,
            512 * 1024,
            addr_0.clone(),
            vec![target_0.clone()],
            tx,
            true,
        );
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            info!("node 0 broadcast message");
            s.broadcast_message("I'm node 0".as_bytes());
        });

        loop {
            if let Ok((sid, data)) = rx.recv() {
                info!(
                    "get message from seesion id {}: {}",
                    sid,
                    str::from_utf8(data.as_ref()).unwrap()
                );
            }
            thread::sleep(Duration::from_secs(1));
        }
    } else {
        info!("Starting node 1 ......");
        let (tx, rx) = unbounded();
        let s = P2P::new(
            path1,
            512 * 1024,
            addr_1.clone(),
            vec![target_1.clone()],
            tx,
            true,
        );
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            info!("node 1 broadcast message");
            s.broadcast_message("I'm node 1".as_bytes());
        });
        loop {
            if let Ok((sid, data)) = rx.recv() {
                info!(
                    "get message from seesion id {}: {}",
                    sid,
                    str::from_utf8(data.as_ref()).unwrap()
                );
            }
            thread::sleep(Duration::from_secs(1));
        }
    }
}
