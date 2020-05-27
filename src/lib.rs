use futures::stream::Stream;
use log::{debug, warn};
use tentacle::{
    builder::{MetaBuilder, ServiceBuilder},
    context::{ProtocolContext, ProtocolContextMutRef, ServiceContext},
    error::Error,
    secio::SecioKeyPair,
    service::{
        DialProtocol, ProtocolHandle, ServiceControl, ServiceError, ServiceEvent, SessionType,
        TargetSession,
    },
    traits::{ServiceHandle, ServiceProtocol},
    utils::{multiaddr_to_socketaddr, socketaddr_to_multiaddr},
    ProtocolId, SessionId,
};

use channel::Sender;
pub use crossbeam_channel as channel;

use parking_lot::RwLock;

use tokio::codec::length_delimited::LengthDelimitedCodec;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::str;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub const PROTOCOL_ID: ProtocolId = ProtocolId::new(0);

#[derive(Clone)]
struct PeersManager {
    outbound_peers: Arc<RwLock<HashMap<usize, SocketAddr>>>,
}

impl PeersManager {
    fn check_addr_exists(&self, addr: &SocketAddr) -> bool {
        for (_, v) in self.outbound_peers.read().iter() {
            if v == addr {
                return true;
            }
        }
        false
    }

    fn add_peer(&self, session_id: usize, peer: &SocketAddr) {
        self.outbound_peers.write().insert(session_id, peer.clone());
    }

    fn remove_peer(&self, session_id: usize) {
        let _ = self.outbound_peers.write().remove_entry(&session_id);
    }
}

struct PHandle {
    tx: Sender<(usize, Vec<u8>)>,
    peers_manager: PeersManager,
}

impl ServiceProtocol for PHandle {
    fn init(&mut self, context: &mut ProtocolContext) {
        debug!("init protocol {}", context.proto_id);
    }

    fn connected(&mut self, context: ProtocolContextMutRef, version: &str) {
        let session = context.session;
        debug!(
            "proto id [{}] open on session [{}], address: [{}], type: [{:?}], version: {}",
            context.proto_id, session.id, session.address, session.ty, version
        );
        if session.ty == SessionType::Outbound {
            if let Some(address) = multiaddr_to_socketaddr(&session.address) {
                self.peers_manager.add_peer(session.id.value(), &address);
            }
        }
    }

    fn disconnected(&mut self, context: ProtocolContextMutRef) {
        debug!(
            "proto id [{}] close on session [{}]",
            context.proto_id, context.session.id
        );
        self.peers_manager.remove_peer(context.session.id.value());
    }

    fn received(&mut self, context: ProtocolContextMutRef, data: bytes::Bytes) {
        debug!(
            "received from [{}]: proto [{}] data len {} data {:?}",
            context.session.id,
            context.proto_id,
            data.len(),
            data
        );
        self.tx
            .send((context.session.id.value(), data.as_ref().to_vec()))
            .unwrap();
    }

    fn notify(&mut self, context: &mut ProtocolContext, token: u64) {
        debug!(
            "proto [{}] received notify token: {}",
            context.proto_id, token
        );
    }
}

struct SHandle {
    peers_manager: PeersManager,
}

impl ServiceHandle for SHandle {
    fn handle_error(&mut self, _context: &mut ServiceContext, error: ServiceError) {
        match error {
            ServiceError::DialerError { address, error } => {
                if let Some(address) = multiaddr_to_socketaddr(&address) {
                    // If dial to a connected node, need add it to connected address list.
                    match error {
                        Error::RepeatedConnection(session_id) => {
                            self.peers_manager.add_peer(session_id.value(), &address);
                        }
                        _ => {
                            warn!("service error: {:?}", error);
                        }
                    }
                }
            }
            _ => {
                warn!("service error: {:?}", error);
            }
        }
    }
    fn handle_event(&mut self, _context: &mut ServiceContext, event: ServiceEvent) {
        debug!("service event: {:?}", event);
    }
}

#[derive(Clone)]
pub struct P2P {
    peers_manager: PeersManager,
    service_control: ServiceControl,
}

impl P2P {
    pub fn new(
        private_key: String,
        frame_len: usize,
        listen_addr: SocketAddr,
        peer_addrs: Vec<SocketAddr>,
        tx: Sender<(usize, Vec<u8>)>,
    ) -> P2P {
        // read and decode private key
        let file = File::open(private_key).expect("cannot open private_key file");
        let mut file = BufReader::new(file);
        let mut private_key_str = String::new();
        let _ = file.read_line(&mut private_key_str);
        // skip 0x
        let private_key_bytes =
            hex::decode(&private_key_str[2..].trim()).expect("failed to decode private key");
        let key_pair = SecioKeyPair::secp256k1_raw_key(private_key_bytes)
            .expect("failed to generate key pair");

        // shared peers manager struct
        let peers_manager = PeersManager {
            outbound_peers: Arc::new(RwLock::new(HashMap::new())),
        };

        // create meta of protocol to transfer
        let peers_manager_clone = peers_manager.clone();
        let meta = MetaBuilder::new()
            .id(PROTOCOL_ID)
            .codec(move || {
                let mut lcodec = LengthDelimitedCodec::new();
                lcodec.set_max_frame_length(frame_len);
                Box::new(lcodec)
            })
            .service_handle(move || {
                let handle = Box::new(PHandle {
                    tx: tx.clone(),
                    peers_manager: peers_manager_clone,
                });
                ProtocolHandle::Callback(handle)
            })
            .build();

        let peers_manager_clone = peers_manager.clone();
        let service = ServiceBuilder::default()
            .insert_protocol(meta)
            .key_pair(key_pair)
            .forever(true)
            .build(SHandle {
                peers_manager: peers_manager_clone,
            });

        let service_control = service.control().clone();

        thread::spawn(move || tokio::run(service.for_each(|_| Ok(()))));

        // start a thread to loop trying to connect
        let control = service_control.clone();
        let peers_manager_clone = peers_manager.clone();
        thread::spawn(move || {
            let listen_addr = socketaddr_to_multiaddr(listen_addr);
            let _ = control.listen(listen_addr);

            loop {
                for addr in peer_addrs.clone() {
                    if !peers_manager_clone.check_addr_exists(&addr) {
                        let _ = control.dial(socketaddr_to_multiaddr(addr), DialProtocol::All);
                    }
                }
                thread::sleep(Duration::from_secs(15));
            }
        });

        P2P {
            peers_manager,
            service_control,
        }
    }

    pub fn send_message(&self, session_id: usize, message: &[u8]) {
        self.service_control
            .send_message_to(
                SessionId::new(session_id),
                PROTOCOL_ID,
                bytes::Bytes::from(message),
            )
            .unwrap();
    }

    pub fn broadcast_message(&self, message: &[u8]) {
        self.service_control
            .filter_broadcast(TargetSession::All, PROTOCOL_ID, bytes::Bytes::from(message))
            .unwrap();
    }
}
