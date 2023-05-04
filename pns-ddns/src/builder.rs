use std::sync::Mutex;

use futures::{channel::oneshot, pin_mut, FutureExt, StreamExt};
use libp2p::PeerId;
use sc_client_api::{BlockBackend, BlockchainEvents, HeaderBackend, ProofProvider};
use sc_network::{
    config::{MultiaddrWithPeerId, SyncMode},
    NetworkBlock, NetworkEventStream, NetworkPeers, NetworkService, NetworkStateInfo,
};
use sc_network_bitswap::BitswapRequestHandler;
use sc_network_common::role::Roles;
use sc_network_light::light_client_requests::handler::LightClientRequestHandler;
use sc_network_sync::{
    block_request_handler::BlockRequestHandler, engine::SyncingEngine,
    service::network::NetworkServiceProvider, state_request_handler::StateRequestHandler,
    warp_request_handler::RequestHandler as WarpSyncRequestHandler, SyncingService,
};
use sc_service::{
    Arc, BuildNetworkParams, Error, ImportQueue, NetworkStarter, Role, TransactionPoolAdapter,
    WarpSyncParams,
};
use sc_transaction_pool_api::MaintainedTransactionPool;

use sc_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};
use sp_api::{offchain::OffchainStorage, HeaderT, ProvideRuntimeApi};
use sp_blockchain::HeaderMetadata;
use sp_consensus::{
    block_validation::{Chain, DefaultBlockAnnounceValidator},
    SyncOracle,
};
use sp_runtime::traits::{Block as BlockT, BlockIdTo};
use tracing::debug;

use crate::network::DdnsNetworkManager;
use crate::offchain::OffChain;

pub struct DdnsNetworkParams<Storage> {
    pub offchain_db: Arc<Mutex<OffChain<Storage>>>,
    pub manager: DdnsNetworkManager,
}

// let ddns_request_protocol_config = {
//     let DdnsNetworkParams {
//         offchain_db,
//         manager,
//     } = ddns_params;
//     let (handler, protocol_config) =
//         crate::network::DdnsReuqestHandler::new(offchain_db, manager);
//     spawn_handle.spawn("ddns-request-handler", Some("networking"), handler.run());
//     protocol_config
// };
/// Build the network service, the network status sinks and an RPC sender.
pub fn build_network<TBl, TExPool, TImpQu, TCl, Storage>(
    ddns_params: DdnsNetworkParams<Storage>,
    params: BuildNetworkParams<TBl, TExPool, TImpQu, TCl>,
) -> Result<
    (
        Arc<NetworkService<TBl, <TBl as BlockT>::Hash>>,
        TracingUnboundedSender<sc_rpc::system::Request<TBl>>,
        sc_network_transactions::TransactionsHandlerController<<TBl as BlockT>::Hash>,
        NetworkStarter,
        Arc<SyncingService<TBl>>,
    ),
    Error,
>
where
    Storage: OffchainStorage + 'static,
    TBl: BlockT,
    TCl: ProvideRuntimeApi<TBl>
        + HeaderMetadata<TBl, Error = sp_blockchain::Error>
        + Chain<TBl>
        + BlockBackend<TBl>
        + BlockIdTo<TBl, Error = sp_blockchain::Error>
        + ProofProvider<TBl>
        + HeaderBackend<TBl>
        + BlockchainEvents<TBl>
        + 'static,
    TExPool: MaintainedTransactionPool<Block = TBl, Hash = <TBl as BlockT>::Hash> + 'static,
    TImpQu: ImportQueue<TBl> + 'static,
{
    let BuildNetworkParams {
        config,
        client,
        transaction_pool,
        spawn_handle,
        import_queue,
        block_announce_validator_builder,
        warp_sync_params,
    } = params;

    let mut request_response_protocol_configs = Vec::new();

    if warp_sync_params.is_none() && config.network.sync_mode.is_warp() {
        return Err("Warp sync enabled, but no warp sync provider configured.".into());
    }

    if client.requires_full_sync() {
        match config.network.sync_mode {
            SyncMode::Fast { .. } => return Err("Fast sync doesn't work for archive nodes".into()),
            SyncMode::Warp => return Err("Warp sync doesn't work for archive nodes".into()),
            SyncMode::Full => {}
        }
    }

    let protocol_id = config.protocol_id();

    let block_announce_validator = if let Some(f) = block_announce_validator_builder {
        f(client.clone())
    } else {
        Box::new(DefaultBlockAnnounceValidator)
    };

    let ddns_request_protocol_config = {
        let DdnsNetworkParams {
            offchain_db,
            manager,
        } = ddns_params;
        let (handler, protocol_config) =
            crate::network::DdnsReuqestHandler::new(offchain_db, manager);
        spawn_handle.spawn("ddns-request-handler", Some("networking"), handler.run());
        protocol_config
    };

    let block_request_protocol_config = {
        // Allow both outgoing and incoming requests.
        let (handler, protocol_config) = BlockRequestHandler::new(
            &protocol_id,
            config.chain_spec.fork_id(),
            client.clone(),
            config.network.default_peers_set.in_peers as usize
                + config.network.default_peers_set.out_peers as usize,
        );
        spawn_handle.spawn("block-request-handler", Some("networking"), handler.run());
        protocol_config
    };

    let state_request_protocol_config = {
        // Allow both outgoing and incoming requests.
        let (handler, protocol_config) = StateRequestHandler::new(
            &protocol_id,
            config.chain_spec.fork_id(),
            client.clone(),
            config.network.default_peers_set_num_full as usize,
        );
        spawn_handle.spawn("state-request-handler", Some("networking"), handler.run());
        protocol_config
    };

    let warp_sync_protocol_config = match warp_sync_params.as_ref() {
        Some(WarpSyncParams::WithProvider(warp_with_provider)) => {
            // Allow both outgoing and incoming requests.
            let (handler, protocol_config) = WarpSyncRequestHandler::new(
                protocol_id.clone(),
                client
                    .block_hash(0u32.into())
                    .ok()
                    .flatten()
                    .expect("Genesis block exists; qed"),
                config.chain_spec.fork_id(),
                warp_with_provider.clone(),
            );
            spawn_handle.spawn(
                "warp-sync-request-handler",
                Some("networking"),
                handler.run(),
            );
            Some(protocol_config)
        }
        _ => None,
    };

    let light_client_request_protocol_config = {
        // Allow both outgoing and incoming requests.
        let (handler, protocol_config) = LightClientRequestHandler::new(
            &protocol_id,
            config.chain_spec.fork_id(),
            client.clone(),
        );
        spawn_handle.spawn(
            "light-client-request-handler",
            Some("networking"),
            handler.run(),
        );
        protocol_config
    };

    let (chain_sync_network_provider, chain_sync_network_handle) = NetworkServiceProvider::new();
    let (engine, sync_service, block_announce_config) = SyncingEngine::new(
        Roles::from(&config.role),
        client.clone(),
        config
            .prometheus_config
            .as_ref()
            .map(|config| config.registry.clone())
            .as_ref(),
        &config.network,
        protocol_id.clone(),
        &config.chain_spec.fork_id().map(ToOwned::to_owned),
        block_announce_validator,
        warp_sync_params,
        chain_sync_network_handle,
        import_queue.service(),
        block_request_protocol_config.name.clone(),
        state_request_protocol_config.name.clone(),
        warp_sync_protocol_config
            .as_ref()
            .map(|config| config.name.clone()),
    )?;
    let sync_service_import_queue = sync_service.clone();
    let sync_service = Arc::new(sync_service);

    request_response_protocol_configs.push(config.network.ipfs_server.then(|| {
        let (handler, protocol_config) = BitswapRequestHandler::new(client.clone());
        spawn_handle.spawn("bitswap-request-handler", Some("networking"), handler.run());
        protocol_config
    }));

    let mut network_params = sc_network::config::Params {
        role: config.role.clone(),
        executor: {
            let spawn_handle = Clone::clone(&spawn_handle);
            Box::new(move |fut| {
                spawn_handle.spawn("libp2p-node", Some("networking"), fut);
            })
        },
        network_config: config.network.clone(),
        chain: client.clone(),
        protocol_id: protocol_id.clone(),
        fork_id: config.chain_spec.fork_id().map(ToOwned::to_owned),
        metrics_registry: config
            .prometheus_config
            .as_ref()
            .map(|config| config.registry.clone()),
        block_announce_config,
        request_response_protocol_configs: request_response_protocol_configs
            .into_iter()
            .chain([
                Some(ddns_request_protocol_config),
                Some(block_request_protocol_config),
                Some(state_request_protocol_config),
                Some(light_client_request_protocol_config),
                warp_sync_protocol_config,
            ])
            .flatten()
            .collect::<Vec<_>>(),
    };

    // crate transactions protocol and add it to the list of supported protocols of `network_params`
    let transactions_handler_proto = sc_network_transactions::TransactionsHandlerPrototype::new(
        protocol_id.clone(),
        client
            .block_hash(0u32.into())
            .ok()
            .flatten()
            .expect("Genesis block exists; qed"),
        config.chain_spec.fork_id(),
    );
    network_params
        .network_config
        .extra_sets
        .insert(0, transactions_handler_proto.set_config());

    let has_bootnodes = !network_params.network_config.boot_nodes.is_empty();
    let network_mut = sc_network::NetworkWorker::new(network_params)?;
    let network = network_mut.service().clone();
    #[allow(dead_code)]
    struct TransactionPoolAdapterProxy<C, P> {
        pool: Arc<P>,
        client: Arc<C>,
    }

    let adapter = TransactionPoolAdapterProxy {
        pool: transaction_pool,
        client: client.clone(),
    };

    let pool = unsafe {
        std::mem::transmute::<
            TransactionPoolAdapterProxy<TCl, TExPool>,
            TransactionPoolAdapter<TCl, TExPool>,
        >(adapter)
    };

    let (tx_handler, tx_handler_controller) = transactions_handler_proto.build(
        network.clone(),
        sync_service.clone(),
        Arc::new(pool),
        config
            .prometheus_config
            .as_ref()
            .map(|config| &config.registry),
    )?;

    spawn_handle.spawn(
        "network-transactions-handler",
        Some("networking"),
        tx_handler.run(),
    );
    spawn_handle.spawn(
        "chain-sync-network-service-provider",
        Some("networking"),
        chain_sync_network_provider.run(network.clone()),
    );
    spawn_handle.spawn(
        "import-queue",
        None,
        import_queue.run(Box::new(sync_service_import_queue)),
    );

    let event_stream = network.event_stream("syncing");
    spawn_handle.spawn("syncing", None, engine.run(event_stream));

    let (system_rpc_tx, system_rpc_rx) = tracing_unbounded("mpsc_system_rpc", 10_000);
    spawn_handle.spawn(
        "system-rpc-handler",
        Some("networking"),
        build_system_rpc_future(
            config.role.clone(),
            network_mut.service().clone(),
            sync_service.clone(),
            client.clone(),
            system_rpc_rx,
            has_bootnodes,
        ),
    );

    let future = build_network_future(
        network_mut,
        client,
        sync_service.clone(),
        config.announce_block,
    );

    // TODO: Normally, one is supposed to pass a list of notifications protocols supported by the
    // node through the `NetworkConfiguration` struct. But because this function doesn't know in
    // advance which components, such as GrandPa or Polkadot, will be plugged on top of the
    // service, it is unfortunately not possible to do so without some deep refactoring. To bypass
    // this problem, the `NetworkService` provides a `register_notifications_protocol` method that
    // can be called even after the network has been initialized. However, we want to avoid the
    // situation where `register_notifications_protocol` is called *after* the network actually
    // connects to other peers. For this reason, we delay the process of the network future until
    // the user calls `NetworkStarter::start_network`.
    //
    // This entire hack should eventually be removed in favour of passing the list of protocols
    // through the configuration.
    //
    // See also https://github.com/paritytech/substrate/issues/6827
    let (network_start_tx, network_start_rx) = oneshot::channel();

    // The network worker is responsible for gathering all network messages and processing
    // them. This is quite a heavy task, and at the time of the writing of this comment it
    // frequently happens that this future takes several seconds or in some situations
    // even more than a minute until it has processed its entire queue. This is clearly an
    // issue, and ideally we would like to fix the network future to take as little time as
    // possible, but we also take the extra harm-prevention measure to execute the networking
    // future using `spawn_blocking`.
    spawn_handle.spawn_blocking("network-worker", Some("networking"), async move {
        if network_start_rx.await.is_err() {
            tracing::warn!(
                "The NetworkStart returned as part of `build_network` has been silently dropped"
            );
            // This `return` might seem unnecessary, but we don't want to make it look like
            // everything is working as normal even though the user is clearly misusing the API.
            return;
        }

        future.await
    });

    Ok((
        network,
        system_rpc_tx,
        tx_handler_controller,
        NetworkStarter::new(network_start_tx),
        sync_service.clone(),
    ))
}

/// Builds a future that continuously polls the network.
async fn build_network_future<
    B: BlockT,
    C: BlockchainEvents<B>
        + HeaderBackend<B>
        + BlockBackend<B>
        + HeaderMetadata<B, Error = sp_blockchain::Error>
        + ProofProvider<B>
        + Send
        + Sync
        + 'static,
    H: sc_network_common::ExHashT,
>(
    network: sc_network::NetworkWorker<B, H>,
    client: Arc<C>,
    sync_service: Arc<SyncingService<B>>,
    announce_imported_blocks: bool,
) {
    let mut imported_blocks_stream = client.import_notification_stream().fuse();

    // Stream of finalized blocks reported by the client.
    let mut finality_notification_stream = client.finality_notification_stream().fuse();

    let network_run = network.run().fuse();
    pin_mut!(network_run);

    loop {
        futures::select! {
            // List of blocks that the client has imported.
            notification = imported_blocks_stream.next() => {
                let notification = match notification {
                    Some(n) => n,
                    // If this stream is shut down, that means the client has shut down, and the
                    // most appropriate thing to do for the network future is to shut down too.
                    None => {
                        debug!("Block import stream has terminated, shutting down the network future.");
                        return
                    },
                };

                if announce_imported_blocks {
                    sync_service.announce_block(notification.hash, None);
                }

                if notification.is_new_best {
                    sync_service.new_best_block_imported(
                        notification.hash,
                        *notification.header.number(),
                    );
                }
            }

            // List of blocks that the client has finalized.
            notification = finality_notification_stream.select_next_some() => {
                sync_service.on_block_finalized(notification.hash, notification.header);
            }

            // Drive the network. Shut down the network future if `NetworkWorker` has terminated.
            _ = network_run => {
                debug!("`NetworkWorker` has terminated, shutting down the network future.");
                return
            }
        }
    }
}

#[test]
fn test() {}

// /// Transaction pool adapter.
// pub struct TransactionPoolAdapter<C, P> {
//     pool: Arc<P>,
//     client: Arc<C>,
// }

/// Builds a future that processes system RPC requests.
async fn build_system_rpc_future<
    B: BlockT,
    C: BlockchainEvents<B>
        + HeaderBackend<B>
        + BlockBackend<B>
        + HeaderMetadata<B, Error = sp_blockchain::Error>
        + ProofProvider<B>
        + Send
        + Sync
        + 'static,
    H: sc_network_common::ExHashT,
>(
    role: Role,
    network_service: Arc<sc_network::NetworkService<B, H>>,
    sync_service: Arc<SyncingService<B>>,
    client: Arc<C>,
    mut rpc_rx: TracingUnboundedReceiver<sc_rpc::system::Request<B>>,
    should_have_peers: bool,
) {
    // Current best block at initialization, to report to the RPC layer.
    let starting_block = client.info().best_number;

    loop {
        // Answer incoming RPC requests.
        let Some(req) = rpc_rx.next().await else {
			debug!("RPC requests stream has terminated, shutting down the system RPC future.");
			return;
		};

        match req {
            sc_rpc::system::Request::Health(sender) => match sync_service.peers_info().await {
                Ok(info) => {
                    let _ = sender.send(sc_rpc::system::Health {
                        peers: info.len(),
                        is_syncing: sync_service.is_major_syncing(),
                        should_have_peers,
                    });
                }
                Err(_) => tracing::error!("`SyncingEngine` shut down"),
            },
            sc_rpc::system::Request::LocalPeerId(sender) => {
                let _ = sender.send(network_service.local_peer_id().to_base58());
            }
            sc_rpc::system::Request::LocalListenAddresses(sender) => {
                let peer_id = (network_service.local_peer_id()).into();
                let p2p_proto_suffix = sc_network::multiaddr::Protocol::P2p(peer_id);
                let addresses = network_service
                    .listen_addresses()
                    .iter()
                    .map(|addr| addr.clone().with(p2p_proto_suffix.clone()).to_string())
                    .collect();
                let _ = sender.send(addresses);
            }
            sc_rpc::system::Request::Peers(sender) => match sync_service.peers_info().await {
                Ok(info) => {
                    let _ = sender.send(
                        info.into_iter()
                            .map(|(peer_id, p)| sc_rpc::system::PeerInfo {
                                peer_id: peer_id.to_base58(),
                                roles: format!("{:?}", p.roles),
                                best_hash: p.best_hash,
                                best_number: p.best_number,
                            })
                            .collect(),
                    );
                }
                Err(_) => tracing::error!("`SyncingEngine` shut down"),
            },
            sc_rpc::system::Request::NetworkState(sender) => {
                let network_state = network_service.network_state().await;
                if let Ok(network_state) = network_state {
                    if let Ok(network_state) = serde_json::to_value(network_state) {
                        let _ = sender.send(network_state);
                    }
                } else {
                    break;
                }
            }
            sc_rpc::system::Request::NetworkAddReservedPeer(peer_addr, sender) => {
                let result = match MultiaddrWithPeerId::try_from(peer_addr) {
                    Ok(peer) => network_service.add_reserved_peer(peer),
                    Err(err) => Err(err.to_string()),
                };
                let x = result.map_err(sc_rpc::system::error::Error::MalformattedPeerArg);
                let _ = sender.send(x);
            }
            sc_rpc::system::Request::NetworkRemoveReservedPeer(peer_id, sender) => {
                let _ = match peer_id.parse::<PeerId>() {
                    Ok(peer_id) => {
                        network_service.remove_reserved_peer(peer_id);
                        sender.send(Ok(()))
                    }
                    Err(e) => sender.send(Err(sc_rpc::system::error::Error::MalformattedPeerArg(
                        e.to_string(),
                    ))),
                };
            }
            sc_rpc::system::Request::NetworkReservedPeers(sender) => {
                let reserved_peers = network_service.reserved_peers().await;
                if let Ok(reserved_peers) = reserved_peers {
                    let reserved_peers = reserved_peers
                        .iter()
                        .map(|peer_id| peer_id.to_base58())
                        .collect();
                    let _ = sender.send(reserved_peers);
                } else {
                    break;
                }
            }
            sc_rpc::system::Request::NodeRoles(sender) => {
                use sc_rpc::system::NodeRole;

                let node_role = match role {
                    Role::Authority { .. } => NodeRole::Authority,
                    Role::Full => NodeRole::Full,
                };

                let _ = sender.send(vec![node_role]);
            }
            sc_rpc::system::Request::SyncState(sender) => {
                use sc_rpc::system::SyncState;

                match sync_service.best_seen_block().await {
                    Ok(best_seen_block) => {
                        let best_number = client.info().best_number;
                        let _ = sender.send(SyncState {
                            starting_block,
                            current_block: best_number,
                            highest_block: best_seen_block.unwrap_or(best_number),
                        });
                    }
                    Err(_) => tracing::error!("`SyncingEngine` shut down"),
                }
            }
        }
    }

    debug!("`NetworkWorker` has terminated, shutting down the system RPC future.");
}
