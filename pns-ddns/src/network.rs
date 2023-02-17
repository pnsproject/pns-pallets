use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{channel::mpsc, StreamExt};
use libp2p::PeerId;
use sc_network_common::request_responses::{IncomingRequest, OutgoingResponse, ProtocolConfig};
use serde::{Deserialize, Serialize};
use sp_api::offchain::OffchainStorage;
use tracing::error;

use crate::offchain::OffChain;

const MAX_REQUEST_QUEUE: usize = 20;
const MAX_PACKET_SIZE: u64 = 16 * 1024 * 1024;

pub const PROTOCOL_NAME: &'static str = "/ddns/0.1";

pub struct DdnsReuqestHandler<Storage> {
    request_receiver: mpsc::Receiver<IncomingRequest>,
    offchain_db: Arc<Mutex<OffChain<Storage>>>,
    manager: DdnsNetworkManager,
}

impl<Storage> DdnsReuqestHandler<Storage>
where
    Storage: OffchainStorage,
{
    pub fn new(
        offchain_db: Arc<Mutex<OffChain<Storage>>>,
        manager: DdnsNetworkManager,
    ) -> (Self, ProtocolConfig) {
        let (tx, request_receiver) = mpsc::channel(MAX_REQUEST_QUEUE);

        let config = ProtocolConfig {
            name: sc_network_common::protocol::ProtocolName::from(PROTOCOL_NAME),
            fallback_names: vec![],
            max_request_size: MAX_PACKET_SIZE,
            max_response_size: MAX_PACKET_SIZE,
            request_timeout: Duration::from_secs(15),
            inbound_queue: Some(tx),
        };

        (
            Self {
                offchain_db,
                request_receiver,
                manager,
            },
            config,
        )
    }

    pub async fn run(mut self) {
        while let Some(request) = self.request_receiver.next().await {
            let IncomingRequest {
                peer,
                payload,
                pending_response,
            } = request;

            match self.handle_message(payload, peer) {
                Ok(response) => {
                    let response = OutgoingResponse {
                        result: Ok(response),
                        reputation_changes: Vec::new(),
                        sent_feedback: None,
                    };

                    if let Err(e) = pending_response.send(response) {
                        error!(target: "ddns_reuqest_handler", "Failed to send response: {:?}", e);
                    }
                }
                Err(err) => {
                    error!(target: "ddns_reuqest_handler", "Failed to handle request: {:?}", err);
                    let response = OutgoingResponse {
                        result: Err(()),
                        reputation_changes: Vec::new(),
                        sent_feedback: None,
                    };

                    if let Err(e) = pending_response.send(response) {
                        error!(target: "ddns_reuqest_handler", "Failed to send response: {:?}", e);
                    }
                }
            }
        }
    }

    fn handle_message(&mut self, payload: Vec<u8>, peer: PeerId) -> Result<Vec<u8>, Error> {
        let (message, _) =
            bincode::serde::decode_from_slice::<Message, _>(&payload, bincode::config::standard())
                .map_err(|_| Error::DecodeFailed)?;
        let response = match message {
            Message::Set { k, v, timestamp } => {
                let mut db = self
                    .offchain_db
                    .lock()
                    .map_err(|_| Error::LockedStorageError)?;
                db.set(&k, &v, timestamp);
                vec![]
            }
            Message::Init => {
                let mut peers = self
                    .manager
                    .peers
                    .lock()
                    .map_err(|_| Error::LockedManagerError)?;
                let response = peers.iter().cloned().collect::<Vec<_>>();
                peers.insert(peer);

                let res = bincode::serde::encode_to_vec(response, bincode::config::standard())
                    .map_err(|_| Error::EncodeFailed)?;
                res
            }
        };

        Ok(response)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Set {
        k: Vec<u8>,
        v: Vec<u8>,
        timestamp: i64,
    },
    Init,
}

impl Message {
    pub fn encode(self) -> Result<Vec<u8>, Error> {
        bincode::serde::encode_to_vec(self, bincode::config::standard())
            .map_err(|_| Error::EncodeFailed)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown request message decode error")]
    DecodeFailed,
    #[error("unknown request message encode error")]
    EncodeFailed,
    #[error("locked storage error")]
    LockedStorageError,
    #[error("locked network manager error")]
    LockedManagerError,
}

#[derive(Default)]
pub struct DdnsNetworkManager {
    pub peers: Arc<Mutex<HashSet<PeerId>>>,
}

impl Clone for DdnsNetworkManager {
    fn clone(&self) -> Self {
        Self {
            peers: self.peers.clone(),
        }
    }
}
