mod block_chain;

use core::{marker::PhantomData, str::FromStr};
use std::net::SocketAddr;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use block_chain::BlockChainAuthority;
use pns_registrar::registrar::BalanceOf;
use pns_registrar::traits::Label;
use pns_runtime_api::PnsStorageApi;
use pns_types::DomainHash;
use sp_api::{BlockId, BlockT, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use tokio::net::UdpSocket;
use tracing::{debug, error, info};
use trust_dns_server::authority::LookupError;
use trust_dns_server::proto::op::ResponseCode;
use trust_dns_server::proto::rr::RData;
use trust_dns_server::{
    authority::{AuthorityObject, Catalog},
    client::rr::LowerName,
    proto::rr::{Name, RecordType},
    ServerFuture,
};

pub struct ServerDeps<Client, Block, Config> {
    pub client: Arc<Client>,
    pub socket: SocketAddr,
    _block: PhantomData<(Block, Config)>,
}

impl<Client, Block, Config> Clone for ServerDeps<Client, Block, Config> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            socket: self.socket.clone(),
            _block: PhantomData::default(),
        }
    }
}

unsafe impl<Client, Block, Config> Send for ServerDeps<Client, Block, Config> where Client: Send {}
unsafe impl<Client, Block, Config> Sync for ServerDeps<Client, Block, Config> where Client: Sync {}

// unsafe impl Send for ServerDeps<(), (), ()> {}
// unsafe impl Sync for ServerDeps<(), (), ()> {}

// impl ServerDeps<(), (), ()> {
//     pub async fn init_dns_server_test(self) {
//         let zone_name = Name::from_str("dot").unwrap();
//         let authority = BlockChainAuthority {
//             origin: LowerName::from(&zone_name),
//             zone_type: trust_dns_server::authority::ZoneType::Primary,
//             inner: self,
//         };

//         let mut catalog: Catalog = Catalog::new();
//         catalog.upsert(
//             LowerName::from(&zone_name),
//             Box::new(Arc::new(authority)) as Box<dyn AuthorityObject>,
//         );

//         let mut server = ServerFuture::new(catalog);

//         let udp_socket = UdpSocket::bind(("127.0.0.1", 65353))
//             .await
//             .expect("bind udp socket failed.");
//         server.register_socket(udp_socket);
//         match server.block_until_done().await {
//             Ok(()) => {
//                 // we're exiting for some reason...
//                 info!("Trust-DNS stopping");
//             }
//             Err(e) => {
//                 error!("Trust-DNS has encountered an error: {e:?}");
//                 panic!("error: {e:?}");
//             }
//         };
//     }
// }

impl<Client, Block, Config> ServerDeps<Client, Block, Config> {
    pub fn new(client: Arc<Client>, socket: impl Into<SocketAddr>) -> Self {
        Self {
            client,
            socket: socket.into(),
            _block: PhantomData::default(),
        }
    }

    pub fn test(client: Client) -> Self {
        Self {
            client: Arc::new(client),
            socket: SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                8080,
            ),
            _block: PhantomData::default(),
        }
    }
}

impl<Client, Block, Config> ServerDeps<Client, Block, Config>
where
    Client: ProvideRuntimeApi<Block>,
    Client: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    Client: Send + Sync + 'static,
    Config: pns_registrar::registrar::Config,
    Client::Api: BlockBuilder<Block>,
    Client::Api: PnsStorageApi<Block, Config::Moment, BalanceOf<Config>>,
    Block: BlockT,
{
    pub async fn init_server(self) {
        let Self { client, socket, .. } = self;

        let app = Router::new()
            .route("/get_info/:id", get(Self::get_info))
            .route("/info/:name", get(Self::get_info_from_name))
            .route("/all", get(Self::all))
            .with_state(client);

        axum::Server::bind(&socket)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }

    pub async fn init_dns_server(self) {
        let zone_name = Name::from_str("dot").unwrap();
        let authority = BlockChainAuthority {
            origin: LowerName::from(&zone_name),
            zone_type: trust_dns_server::authority::ZoneType::Primary,
            inner: self,
        };

        let mut catalog: Catalog = Catalog::new();
        catalog.upsert(
            LowerName::from(&zone_name),
            Box::new(Arc::new(authority)) as Box<dyn AuthorityObject>,
        );

        let mut server = ServerFuture::new(catalog);

        let udp_socket = UdpSocket::bind(("127.0.0.1", 25353))
            .await
            .expect("bind udp socket failed.");
        server.register_socket(udp_socket);
        match server.block_until_done().await {
            Ok(()) => {
                // we're exiting for some reason...
                info!("Trust-DNS stopping");
            }
            Err(e) => {
                error!("Trust-DNS has encountered an error: {e:?}");
                panic!("error: {e:?}");
            }
        };
    }

    async fn get_info(
        State(client): State<Arc<Client>>,
        Path(id): Path<DomainHash>,
    ) -> impl IntoResponse {
        let at = client.info().best_hash;
        let api = client.runtime_api();
        let res = match api.get_info(&BlockId::hash(at), id) {
            Ok(res) => res,
            Err(e) => {
                tracing::error!("get info error: {e:?}");
                None
            }
        };

        if res.is_none() {
            tracing::info!("query id: {id:?} not found info.");
        }

        Json(res)
    }

    pub(crate) fn inner_lookup(
        &self,
        name: &Name,
    ) -> Result<Vec<(RecordType, RData)>, LookupError> {
        let at = self.client.info().best_hash;
        let api = self.client.runtime_api();
        let id = name_hash(name);

        match api.lookup(&BlockId::hash(at), id) {
            Ok(res) => {
                let mut records = Vec::new();
                for (raw_tp, v) in res.into_iter() {
                    let rt = RecordType::from(raw_tp);
                    let rdata = bincode::serde::decode_from_slice::<RData, _>(
                        &v,
                        bincode::config::legacy(),
                    )
                    .map_err(|_| LookupError::ResponseCode(ResponseCode::FormErr))?
                    .0;
                    records.push((rt, rdata));
                }
                Ok(records)
            }
            Err(err) => {
                error!("lookup {name} failed: {err:?}");
                Err(LookupError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    err,
                )))
            }
        }
    }

    async fn get_info_from_name(
        State(client): State<Arc<Client>>,
        Path(name): Path<String>,
    ) -> impl IntoResponse {
        let at = client.info().best_hash;
        let api = client.runtime_api();
        let res = Label::new_with_len(name.as_bytes())
            .map(|(label, _)| {
                use sp_core::Get;
                let basenode = <Config as pns_registrar::registrar::Config>::BaseNode::get();
                label.encode_with_node(&basenode)
            })
            .and_then(|id| match api.get_info(&BlockId::hash(at), id) {
                Ok(res) => {
                    if res.is_none() {
                        tracing::info!("query id: {id:?} not found info.");
                    }
                    res
                }
                Err(e) => {
                    tracing::error!("get info error: {e:?}");
                    None
                }
            });

        Json(res)
    }

    async fn all(State(client): State<Arc<Client>>) -> impl IntoResponse {
        let at = client.info().best_hash;
        let api = client.runtime_api();
        let res = match api.all(&BlockId::hash(at)) {
            Ok(res) => res,
            Err(e) => {
                tracing::error!("get info error: {e:?}");
                Vec::new()
            }
        };

        Json(res)
    }
}

fn name_hash(name: &Name) -> DomainHash {
    error!("name_hash {name:?}");
    let mut iter = name.iter();
    let base = iter.next_back().expect("not found base label");
    error!("base: {:?}", base);
    iter.fold(Option::<Label>::None, |init, label| {
        if let Some(init) = init {
            Some(init.encode_with_name(label).expect("new label failed."))
        } else {
            Some(
                // TODO: handle error
                Label::new(label).expect("new label failed."),
            )
        }
    })
    .and_then(|label| label.encode_with_basename(base))
    .unwrap_or(Label::new_basenode(base).expect("new basenode faield."))
    .node
}
