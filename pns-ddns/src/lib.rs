mod block_chain;
mod offchain;

use core::{marker::PhantomData, str::FromStr};
use std::net::SocketAddr;

use std::sync::Arc;

use axum::routing::get;
use axum::{
    extract::{Path, State},
    routing::post,
};
use axum::{http::StatusCode, response::IntoResponse};
use axum::{Json, Router};
use block_chain::BlockChainAuthority;
use pns_registrar::registrar::BalanceOf;
use pns_registrar::traits::Label;
use pns_runtime_api::PnsStorageApi;
use pns_types::DomainHash;
use sc_client_api::backend::Backend as BackendT;
use sp_api::{BlockId, BlockT, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_core::Pair;
use tokio::net::UdpSocket;
use tracing::{error, info};
use trust_dns_server::authority::LookupError;
use trust_dns_server::proto::op::ResponseCode;
pub use trust_dns_server::proto::rr::Name;
pub use trust_dns_server::proto::rr::RData;
use trust_dns_server::{
    authority::{AuthorityObject, Catalog},
    client::rr::LowerName,
    proto::rr::RecordType,
    ServerFuture,
};

pub struct ServerDeps<Client, Backend, Block, Config> {
    pub client: Arc<Client>,
    pub backend: Arc<Backend>,
    pub socket: SocketAddr,
    _block: PhantomData<(Block, Config)>,
}

impl<Client, Backend, Block, Config> Clone for ServerDeps<Client, Backend, Block, Config> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            backend: self.backend.clone(),
            socket: self.socket.clone(),
            _block: PhantomData::default(),
        }
    }
}

unsafe impl<Client, Backend, Block, Config> Send for ServerDeps<Client, Backend, Block, Config> where
    Client: Send
{
}
unsafe impl<Client, Backend, Block, Config> Sync for ServerDeps<Client, Backend, Block, Config> where
    Client: Sync
{
}

impl<Client, Backend, Block, Config> ServerDeps<Client, Backend, Block, Config> {
    pub fn new(client: Arc<Client>, backend: Arc<Backend>, socket: impl Into<SocketAddr>) -> Self {
        Self {
            client,
            backend,
            socket: socket.into(),
            _block: PhantomData::default(),
        }
    }

    pub fn test(client: Client, backend: Backend) -> Self {
        Self {
            client: Arc::new(client),
            backend: Arc::new(backend),
            socket: SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                8080,
            ),
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
    pub async fn init_server(self) {
        let Self {
            client,
            socket,
            backend,
            ..
        } = self;

        let app = Router::new()
            .route("/get_info/:id", get(Self::get_info))
            .route("/info/:name", get(Self::get_info_from_name))
            .route("/set_record/:data", post(Self::set_record))
            .route("/all", get(Self::all))
            .with_state(DdnsState { client, backend });

        axum::Server::bind(&socket)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }

    pub async fn init_dns_server(self) {
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

    async fn set_record(
        State(state): State<DdnsState<Client, Backend>>,
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
            match api.check_node_useable(&BlockId::hash(at), id, who) {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("get info error: {e:?}");
                    false
                }
            }
        };

        let res = offchain::OffChain::set_with_signature::<Config, _>(
            who, code, id, tp, content, checker,
        );

        if !res {
            tracing::info!("set id: {id:?} falied.");
        }

        (StatusCode::ACCEPTED, Json(res))
    }
    async fn get_info(
        State(state): State<DdnsState<Client, Backend>>,
        Path(id): Path<DomainHash>,
    ) -> impl IntoResponse {
        let client = state.client;
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
        let id = name_hash(name).ok_or(LookupError::ResponseCode(ResponseCode::NoError))?;
        info!("namehash: {id:?}");
        match api.lookup(&BlockId::hash(at), id) {
            Ok(mut onchain) => {
                let mut offchain = offchain::OffChain::get::<Config>(id);
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
        State(state): State<DdnsState<Client, Backend>>,
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

    async fn all(State(state): State<DdnsState<Client, Backend>>) -> impl IntoResponse {
        let client = state.client;
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

pub struct DdnsState<Client, Backend> {
    pub client: Arc<Client>,
    pub backend: Arc<Backend>,
}

impl<Client, Backend> Clone for DdnsState<Client, Backend> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            backend: self.backend.clone(),
        }
    }
}
