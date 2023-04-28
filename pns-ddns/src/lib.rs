mod block_chain;
mod builder;
mod network;
mod offchain;

use core::{marker::PhantomData, str::FromStr};
use std::{net::SocketAddr, sync::Mutex, time::Duration};

use std::sync::Arc;

pub use crate::builder::{build_network, DdnsNetworkParams};
pub use crate::network::{DdnsNetworkManager, DdnsReuqestHandler};
pub use crate::offchain::{from_backend, OffChain};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use block_chain::BlockChainAuthority;
use libp2p::PeerId;
use network::Message;
use pns_registrar::{registrar::BalanceOf, traits::Label};
use pns_runtime_api::PnsStorageApi;
use pns_types::DomainHash;
use sc_client_api::backend::Backend as BackendT;
use sc_network::NetworkRequest;
use sc_service::SpawnTaskHandle;
use sp_api::{BlockT, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_core::Pair;
use tokio::net::UdpSocket;
use tracing::{error, info, warn};

pub use trust_dns_server::proto::rr::{Name, RData};
use trust_dns_server::{
    authority::{AuthorityObject, Catalog, LookupError},
    client::rr::LowerName,
    proto::{op::ResponseCode, rr::RecordType},
    ServerFuture,
};

pub struct ServerDeps<Client, Backend, Block, Config>
where
    Block: BlockT,
    Backend: BackendT<Block>,
{
    pub client: Arc<Client>,
    pub backend: Arc<Backend>,
    pub offchain_db: Arc<Mutex<OffChain<<Backend as BackendT<Block>>::OffchainStorage>>>,
    pub manager: DdnsNetworkManager,
    pub network: Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
    pub spawn_handle: SpawnTaskHandle,
    _block: PhantomData<(Block, Config)>,
}

impl<Client, Backend, Block, Config> Clone for ServerDeps<Client, Backend, Block, Config>
where
    Block: BlockT,
    Backend: BackendT<Block>,
{
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            backend: self.backend.clone(),
            manager: self.manager.clone(),
            network: self.network.clone(),
            spawn_handle: self.spawn_handle.clone(),
            _block: PhantomData::default(),
            offchain_db: self.offchain_db.clone(),
        }
    }
}

unsafe impl<Client, Backend, Block, Config> Send for ServerDeps<Client, Backend, Block, Config>
where
    Client: Send,
    Block: BlockT,
    Backend: BackendT<Block>,
{
}
unsafe impl<Client, Backend, Block, Config> Sync for ServerDeps<Client, Backend, Block, Config>
where
    Client: Sync,
    Block: BlockT,
    Backend: BackendT<Block>,
{
}

impl<Client, Backend, Block, Config> ServerDeps<Client, Backend, Block, Config>
where
    Block: BlockT,
    Backend: BackendT<Block>,
{
    pub fn new(
        client: Arc<Client>,
        backend: Arc<Backend>,
        manager: DdnsNetworkManager,
        network: Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
        offchain_db: Arc<Mutex<OffChain<<Backend as BackendT<Block>>::OffchainStorage>>>,
        spawn_handle: SpawnTaskHandle,
    ) -> Self {
        Self {
            client,
            offchain_db,
            backend,
            manager,
            spawn_handle,
            network,
            _block: PhantomData::default(),
        }
    }
}

impl<Client, Backend, Block, Config> ServerDeps<Client, Backend, Block, Config>
where
    Client: ProvideRuntimeApi<Block>,
    Client: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    Client: Send + Sync + 'static,
    Config: pns_registrar::registrar::Config + pns_resolvers::resolvers::Config,
    Client::Api: PnsStorageApi<
        Block,
        Config::Moment,
        BalanceOf<Config>,
        Config::Signature,
        Config::AccountId,
    >,
    Client::Api: BlockBuilder<Block>,
    Block: BlockT,
    Backend: BackendT<Block> + 'static,
{
    pub async fn init_server(self, socket: impl Into<SocketAddr>) {
        let socket = socket.into();

        let app = Router::new()
            .route("/get_info/:id", get(Self::get_info))
            .route("/info/:name", get(Self::get_info_from_name))
            .route("/set_record/:data", post(Self::set_record))
            .route("/all", get(Self::all))
            .route("/ddns/state", get(Self::ddns_state))
            .with_state(self);

        axum::Server::bind(&socket)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }

    pub async fn init_dns_server(self, port: u16) {
        let zone_name = Name::from_str("dot").unwrap();
        let authority = BlockChainAuthority {
            origin: LowerName::from(&zone_name),
            root: Name::root().into(),
            zone_type: trust_dns_server::authority::ZoneType::Primary,
            inner: self,
        };

        let mut catalog: Catalog = Catalog::new();
        catalog.upsert(
            LowerName::from(&zone_name),
            Box::new(Arc::new(authority)) as Box<dyn AuthorityObject>,
        );

        let mut server = ServerFuture::new(catalog);

        let udp_socket = UdpSocket::bind(("127.0.0.1", port))
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

    async fn set_record(
        State(state): State<Self>,
        Path(hex_data): Path<String>,
    ) -> impl IntoResponse {
        let Ok(bytes) = hex::decode(&hex_data) else {
            error!("invalid hex data: {hex_data:?}");
            return (StatusCode::BAD_REQUEST,Json(false));
        };

        let Ok(data) = serde_json::from_slice::<SetCode<Config>>(&bytes) else {
            error!("invalid json data: {bytes:?}");
            return (StatusCode::BAD_REQUEST,Json(false));
        };

        let SetCode {
            who,
            code,
            id,
            tp,
            content,
        } = data;
        let client = state.client;
        let checker = |id: DomainHash, who: &Config::AccountId| -> bool {
            let at = client.info().best_hash;
            let api = client.runtime_api();
            match api.check_node_useable(at, id, who) {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("get info error: {e:?}");
                    false
                }
            }
        };

        // offchain:
        let mut guard = state.offchain_db.lock().expect("db lock error");

        if let Some((k, v)) =
            guard.set_with_signature::<Config, _>(who, code, id, tp, content, checker)
        {
            if let Ok(peers) = state.manager.peers.lock() {
                let msg = Message::Set {
                    k,
                    v,
                    timestamp: chrono::Utc::now().timestamp(),
                };
                if let Ok(request) = msg.encode() {
                    let spawn_handle = state.spawn_handle;
                    let network = state.network;

                    for peer in peers.iter().cloned() {
                        spawn_handle.spawn(
                            "ddns_handle_peer",
                            Some("ddns"),
                            gen_task(network.clone(), request.clone(), peer),
                        );
                    }
                } else {
                    tracing::error!(target: "offchain_worker", "Failed to encode message");
                }
            } else {
                tracing::error!(target: "offchain_worker", "Failed to lock storage");
            }
        } else {
            tracing::info!("set id: {id:?} falied.");
            return (StatusCode::ACCEPTED, Json(false));
        }

        (StatusCode::ACCEPTED, Json(true))
    }
    async fn get_info(State(state): State<Self>, Path(id): Path<DomainHash>) -> impl IntoResponse {
        let client = state.client;
        let at = client.info().best_hash;
        let api = client.runtime_api();
        let res = match api.get_info(at, id) {
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
        let id = name_hash(name).ok_or(LookupError::ResponseCode(ResponseCode::NoError))?;
        info!("namehash: {id:?}");
        match api.lookup(at, id) {
            Ok(mut onchain) => {
                // offchain:
                let mut guard = self.offchain_db.lock().expect("db lock error");
                let mut offchain = guard.get::<Config>(id);

                onchain.append(&mut offchain);
                let mut records = Vec::new();
                for (raw_tp, v) in onchain.into_iter() {
                    let rt = RecordType::from(raw_tp);
                    info!("will serde rdata");
                    let rdata = bincode::serde::decode_from_slice::<RData, _>(
                        &v,
                        bincode::config::legacy(),
                    )
                    .map_err(|_| LookupError::ResponseCode(ResponseCode::FormErr))?
                    .0;
                    info!("serde rdata well");
                    records.push((rt, rdata));
                }
                info!("inner inner_lookup res: {records:?}");
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
        State(state): State<Self>,
        Path(name): Path<String>,
    ) -> impl IntoResponse {
        let client = state.client;
        let at = client.info().best_hash;
        let api = client.runtime_api();
        let res = Label::new_with_len(name.as_bytes())
            .map(|(label, _)| {
                use sp_core::Get;
                let basenode = <Config as pns_registrar::registrar::Config>::BaseNode::get();
                label.encode_with_node(&basenode)
            })
            .and_then(|id| match api.get_info(at, id) {
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

    async fn all(State(state): State<Self>) -> impl IntoResponse {
        let client = state.client;
        let at = client.info().best_hash;
        let api = client.runtime_api();
        let res = match api.all(at) {
            Ok(res) => res,
            Err(e) => {
                tracing::error!("get info error: {e:?}");
                Vec::new()
            }
        };

        Json(res)
    }

    async fn ddns_state(State(state): State<Self>) -> impl IntoResponse {
        let peers = state.manager.peers;
        let lock = peers.lock().expect("failed to lock peers");
        let res = lock.iter().map(|id| id.to_base58()).collect::<Vec<_>>();
        Json(res)
    }
}

pub fn name_hash_str(name: &str) -> Option<DomainHash> {
    let name = Name::from_str(name).ok()?;
    name_hash(&name)
}

fn name_hash(name: &Name) -> Option<DomainHash> {
    error!("name_hash {name:?}");
    let mut iter = name.iter();
    let base = iter.next_back()?;
    error!("base: {:?}", base);
    Some(
        iter.fold(Option::<Label>::None, |init, label| {
            if let Some(init) = init {
                Some(init.encode_with_name(label)?)
            } else {
                Some(
                    // TODO: handle error
                    Label::new(label)?,
                )
            }
        })
        .and_then(|label| label.encode_with_basename(base))
        .unwrap_or(Label::new_basenode(base)?)
        .node,
    )
}

use sp_runtime::traits::IdentifyAccount;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct SetCode<T: pns_resolvers::resolvers::Config> {
    pub who: T::AccountId,
    pub code: T::Signature,
    pub id: DomainHash,
    pub tp: pns_types::ddns::codec_type::RecordType,
    pub content: Vec<u8>,
}

impl<C> SetCode<C>
where
    C: pns_resolvers::resolvers::Config,
{
    pub fn new<P, Public, Signature>(pair: P, id: DomainHash, rdata: RData) -> Self
    where
        P: Pair,
        Public: From<<P as Pair>::Public> + Into<<C as pns_resolvers::resolvers::Config>::Public>,
        Signature:
            From<<P as Pair>::Signature> + Into<<C as pns_resolvers::resolvers::Config>::Signature>,
    {
        let tp = Into::<pns_types::ddns::codec_type::RecordType>::into(rdata.to_record_type());
        let content = bincode::serde::encode_to_vec(rdata.clone(), bincode::config::legacy())
            .expect("bincode encode failed");
        Self::new_raw::<P, Public, Signature>(pair, id, tp, content)
    }
    pub fn new_raw<P, Public, Signature>(
        pair: P,
        id: DomainHash,
        tp: pns_types::ddns::codec_type::RecordType,
        content: Vec<u8>,
    ) -> Self
    where
        P: Pair,
        Public: From<<P as Pair>::Public> + Into<<C as pns_resolvers::resolvers::Config>::Public>,
        Signature:
            From<<P as Pair>::Signature> + Into<<C as pns_resolvers::resolvers::Config>::Signature>,
    {
        let data = sp_api::Encode::encode(&(id, tp, &content));
        let who = Public::from(pair.public()).into().into_account();
        let code = Signature::from(pair.sign(&data)).into();
        Self {
            who,
            code,
            id,
            tp,
            content,
        }
    }
    pub fn hex(&self) -> String {
        let slice = serde_json::to_vec(self).expect("serde json to vec failed.");
        hex::encode(slice)
    }
}

async fn gen_task<Block: BlockT>(
    network: Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
    request: Vec<u8>,
    peer: libp2p::PeerId,
) {
    if let Err(e) = network
        .request(
            peer,
            sc_network::ProtocolName::from(network::PROTOCOL_NAME),
            request,
            sc_network::IfDisconnected::ImmediateError,
        )
        .await
    {
        error!("{e:?}");
    }
}

pub async fn init_ddns<TBl>(
    manager: DdnsNetworkManager,
    network: Arc<sc_network::NetworkService<TBl, <TBl as BlockT>::Hash>>,
) where
    TBl: BlockT,
{
    let request = Message::Init.encode().expect("message encode failed");

    tokio::time::sleep(Duration::from_secs(20)).await;

    if let Ok(state) = network.network_state().await {
        let peers = state.connected_peers;
        for (peer_raw, _) in peers.iter() {
            let peer = PeerId::from_str(peer_raw).expect("peerid from str failed");
            match network
                .request(
                    peer,
                    sc_network::ProtocolName::from(network::PROTOCOL_NAME),
                    request.clone(),
                    sc_network::IfDisconnected::ImmediateError,
                )
                .await
            {
                Ok(response) => {
                    match bincode::serde::decode_from_slice::<Vec<PeerId>, _>(
                        &response,
                        bincode::config::standard(),
                    ) {
                        Ok((list, _)) => {
                            let mut lock =
                                manager.peers.lock().expect("ddns manager lock poisoned");
                            lock.extend(list);
                            lock.insert(peer);
                        }
                        Err(e) => {
                            error!("{e}");
                            continue;
                        }
                    };
                }
                Err(e) => error!("ddns init failed: {e}"),
            }
        }
    } else {
        warn!("get connected_peers falied")
    }
}
