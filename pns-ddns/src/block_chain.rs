use core::{borrow::Borrow, str::FromStr};
use std::net::Ipv4Addr;

use pns_registrar::registrar::BalanceOf;
use pns_runtime_api::PnsStorageApi;
use sp_api::{BlockT, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use std::sync::Arc;
use tracing::{debug, info};
use trust_dns_server::{
    authority::{
        AnyRecords, AuthLookup, Authority, LookupError, LookupOptions, LookupRecords, LookupResult,
        MessageRequest, UpdateResult, ZoneType,
    },
    client::rr::LowerName,
    proto::{
        op::ResponseCode,
        rr::{RData, RecordSet, RecordType},
    },
    server::RequestInfo,
};

use crate::ServerDeps;

pub struct BlockChainAuthority<Client, Block, Config> {
    pub origin: LowerName,
    pub zone_type: ZoneType,
    pub inner: ServerDeps<Client, Block, Config>,
}

#[async_trait::async_trait]
impl<Client, Block, Config> Authority for BlockChainAuthority<Client, Block, Config>
where
    Client: ProvideRuntimeApi<Block>,
    Client: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    Client: Send + Sync + 'static,
    Config: pns_registrar::registrar::Config,
    Client::Api: BlockBuilder<Block>,
    Client::Api: PnsStorageApi<Block, Config::Moment, BalanceOf<Config>>,
    Block: BlockT,
{
    type Lookup = AuthLookup;

    fn zone_type(&self) -> ZoneType {
        self.zone_type
    }

    fn is_axfr_allowed(&self) -> bool {
        false
    }

    async fn update(&self, _update: &MessageRequest) -> UpdateResult<bool> {
        Err(ResponseCode::NotImp)
    }

    fn origin(&self) -> &LowerName {
        &self.origin
    }

    async fn lookup(
        &self,
        name: &LowerName,
        rtype: RecordType,
        lookup_options: LookupOptions,
    ) -> Result<Self::Lookup, LookupError> {
        info!("in lookup.");
        // if !self.origin.zone_of(name) {
        //     return Err(LookupError::ResponseCode(ResponseCode::NotImp));
        // }

        let inner = &self.inner;
        let (result, additionals): (LookupResult<LookupRecords>, Option<LookupRecords>) =
            match rtype {
                RecordType::AXFR | RecordType::ANY => {
                    let res = inner.inner_lookup(name.borrow());
                    // let res = inner.inner_lookup(name.borrow());

                    let rrset = res
                        .into_iter()
                        .map(|(tp, bytes)| {
                            Arc::new({
                                let mut set = RecordSet::new(name.borrow(), rtype, 0);
                                if !set.add_rdata({
                                    match tp {
                                        RecordType::A => RData::A(
                                            // TODO:
                                            Ipv4Addr::from_str(unsafe {
                                                core::str::from_utf8_unchecked(&bytes)
                                            })
                                            .expect("Ipv4 address from str failed."),
                                        ),
                                        _ => todo!(),
                                    }
                                }) {
                                    // TODO:
                                    panic!("insert rdata failed.");
                                };
                                set
                            })
                        })
                        .collect();
                    let result = AnyRecords::new(lookup_options, rrset, rtype, name.clone());
                    (Ok(LookupRecords::AnyRecords(result)), None)
                }
                _ => {
                    return Err(LookupError::ResponseCode(ResponseCode::NotImp));
                }
            };

        result.map(|answers| AuthLookup::answers(answers, additionals))
    }

    async fn search(
        &self,
        request_info: RequestInfo<'_>,
        lookup_options: LookupOptions,
    ) -> Result<Self::Lookup, LookupError> {
        debug!("searching BlockChainAuthority for: {}", request_info.query);
        let name = request_info.query.name();
        let rtype: RecordType = request_info.query.query_type();
        debug!("{name:?} {rtype:?}");
        let inner = &self.inner;
        let (result, additionals): (LookupResult<LookupRecords>, Option<LookupRecords>) =
            match rtype {
                RecordType::AXFR | RecordType::ANY | RecordType::A => {
                    // let res = inner.inner_lookup_test(name.borrow());
                    let res = inner.inner_lookup(name.borrow());

                    let rrset = res
                        .into_iter()
                        .map(|(tp, bytes)| {
                            Arc::new({
                                let mut set = RecordSet::new(name.borrow(), rtype, 0);
                                if !set.add_rdata({
                                    match tp {
                                        RecordType::A => RData::A(
                                            // TODO:
                                            Ipv4Addr::from_str(unsafe {
                                                core::str::from_utf8_unchecked(&bytes)
                                            })
                                            .expect("Ipv4 address from str failed."),
                                        ),
                                        _ => todo!(),
                                    }
                                }) {
                                    // TODO:
                                    panic!("insert rdata failed.");
                                };
                                set
                            })
                        })
                        .collect();
                    let result = AnyRecords::new(lookup_options, rrset, rtype, name.clone());
                    (Ok(LookupRecords::AnyRecords(result)), None)
                }
                _ => return Err(LookupError::ResponseCode(ResponseCode::NotImp)),
            };

        result.map(|answers| AuthLookup::answers(answers, additionals))
    }

    async fn get_nsec_records(
        &self,
        _name: &LowerName,
        _lookup_options: LookupOptions,
    ) -> Result<Self::Lookup, LookupError> {
        Ok(AuthLookup::default())
    }
}

#[cfg(test)]
#[test]
fn name() {
    use core::str::FromStr;

    use trust_dns_server::proto::rr::Name;
    let name = Name::from_str("cupnfish.dot").unwrap();
    println!("{name:?}");
    let origin = Name::from_str("dot").unwrap();
    println!("{}", origin.zone_of(&name));

    let ip_bytes = "127.0.0.1".as_bytes();
    println!("raw: {ip_bytes:?}",);
    let rdata = RData::A(Ipv4Addr::from_str(core::str::from_utf8(ip_bytes).unwrap()).unwrap());
    println!("rdata: {rdata:?}");
}

// #[cfg(test)]
// #[tokio::test]
// async fn test_query() {
//     // a builder for `FmtSubscriber`.
//     let subscriber = tracing_subscriber::FmtSubscriber::builder()
//         // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
//         // will be written to stdout.
//         .with_max_level(tracing::Level::DEBUG)
//         // completes the builder.
//         .finish();

//     tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
//     let server = ServerDeps::<(), (), ()>::test(());
//     server.init_dns_server_test().await;
// }
