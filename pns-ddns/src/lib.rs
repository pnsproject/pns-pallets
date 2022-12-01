use core::marker::PhantomData;
use std::net::SocketAddr;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use pns_registrar::registrar::BalanceOf;
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
    pub fn new(client: Client, socket: impl Into<SocketAddr>) -> Self {
        Self {
            client: Arc::new(client),
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
            .route("/id", get(Self::get_info))
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

        Json(res)
    }
}
