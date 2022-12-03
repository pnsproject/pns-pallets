use core::marker::PhantomData;
use std::net::SocketAddr;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use pns_registrar::registrar::BalanceOf;
use pns_registrar::traits::Label;
use pns_runtime_api::PnsStorageApi;
use pns_types::DomainHash;
use sp_api::{BlockId, BlockT, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};

pub struct ServerDeps<Client, Block, Config> {
    pub client: Arc<Client>,
    pub socket: SocketAddr,
    _block: PhantomData<(Block, Config)>,
}

impl<Client, Block, Config> ServerDeps<Client, Block, Config> {
    pub fn new(client: Arc<Client>, socket: impl Into<SocketAddr>) -> Self {
        Self {
            client,
            socket: socket.into(),
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
            .route("/hello", get(Self::hello))
            .with_state(client);

        axum::Server::bind(&socket)
            .serve(app.into_make_service())
            .await
            .unwrap();
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

    async fn get_info_from_name(
        State(client): State<Arc<Client>>,
        Path(name): Path<String>,
    ) -> impl IntoResponse {
        let at = client.info().best_hash;
        let api = client.runtime_api();
        let res = Label::new(name.as_bytes())
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

    async fn hello() -> &'static str {
        tracing::info!("hello world");

        "Hello, World!"
    }
}

#[test]
fn test() {
    let a = DomainHash::from([
        63, 206, 125, 19, 100, 168, 147, 226, 19, 188, 66, 18, 121, 43, 81, 127, 252, 136, 245,
        177, 59, 134, 200, 239, 156, 141, 57, 12, 58, 19, 112, 206,
    ]);
    println!("{a:?}");

    let label = Label::new("polkasluts".as_bytes()).unwrap().0;
    let a = label.encode_with_node(&a);
    println!("{a:?}");
    println!("0x0007c94a060fa87aa8f41ff6f12cf2b52361d9d5b341ed5fc4d18a477d74f1f2");
}
