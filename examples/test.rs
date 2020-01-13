use channel::unbounded;
use log::info;
use p2p_simple::{channel, P2P};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::str;
use std::thread;
use std::time::Duration;

// run example:
// RUST_LOG=info cargo run --example test 0
// RUST_LOG=info cargo run --example test 1
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
    if std::env::args().nth(1) == Some("0".to_string()) {
        info!("Starting node 0 ......");
        let (tx, rx) = unbounded();
        let s = P2P::new(path0, 512 * 1024, addr_0.clone(), vec![addr_1.clone()], tx);
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
        let s = P2P::new(path1, 512 * 1024, addr_1.clone(), vec![addr_0.clone()], tx);
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
