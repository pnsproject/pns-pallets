use core::borrow::Borrow;
use std::collections::HashSet;

use futures_util::{future, TryFutureExt};
use pns_registrar::registrar::BalanceOf;
use pns_runtime_api::PnsStorageApi;
use sp_api::{BlockT, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use std::sync::Arc;
use tracing::{debug, error, info};
use trust_dns_server::{
    authority::{
        AnyRecords, AuthLookup, Authority, LookupError, LookupOptions, LookupRecords, LookupResult,
        MessageRequest, UpdateResult, ZoneType,
    },
    client::rr::LowerName,
    proto::{
        op::ResponseCode,
        rr::{RData, Record, RecordSet, RecordType},
    },
    resolver::Name,
    server::RequestInfo,
};

use crate::ServerDeps;

pub struct BlockChainAuthority<Client, Block, Config> {
    pub origin: LowerName,
    pub zone_type: ZoneType,
    pub inner: ServerDeps<Client, Block, Config>,
}

impl<Client, Block, Config> BlockChainAuthority<Client, Block, Config>
where
    Client: ProvideRuntimeApi<Block>,
    Client: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    Client: Send + Sync + 'static,
    Config: pns_registrar::registrar::Config,
    Client::Api: BlockBuilder<Block>,
    Client::Api: PnsStorageApi<Block, Config::Moment, BalanceOf<Config>>,
    Block: BlockT,
{
    fn inner_lookup(
        &self,
        name: &LowerName,
        record_type: RecordType,
        lookup_options: LookupOptions,
    ) -> Option<Arc<RecordSet>> {
        let inner = &self.inner;
        let all_res = inner.inner_lookup(name.borrow()).ok()?;
        let lookup = all_res
            .into_iter()
            .find(|(key_type, _)| is_need_type(*key_type, record_type))
            .map(|(key_type, rdata)| {
                Arc::new({
                    let mut set = RecordSet::new(name.borrow(), key_type, 0);
                    if !set.add_rdata(rdata) {
                        // TODO:
                        error!("insert rdata failed.");
                    };
                    set
                })
            });
        // TODO: maybe unwrap this recursion.
        match lookup {
            None => self.inner_lookup_wildcard(name, record_type, lookup_options),
            l => l,
        }
    }

    fn inner_lookup_wildcard(
        &self,
        name: &LowerName,
        record_type: RecordType,
        lookup_options: LookupOptions,
    ) -> Option<Arc<RecordSet>> {
        // if this is a wildcard or a root, both should break continued lookups
        let wildcard = if name.is_wildcard() || name.is_root() {
            return None;
        } else {
            name.clone().into_wildcard()
        };

        self.inner_lookup(&wildcard, record_type, lookup_options)
            // we need to change the name to the query name in the result set since this was a wildcard
            .map(|rrset| {
                let mut new_answer =
                    RecordSet::with_ttl(Name::from(name), rrset.record_type(), rrset.ttl());

                let (records, rrsigs): (Vec<&Record>, Vec<&Record>) = rrset
                    .records(
                        lookup_options.is_dnssec(),
                        lookup_options.supported_algorithms(),
                    )
                    .partition(|r| r.record_type() != RecordType::RRSIG);

                for record in records {
                    if let Some(rdata) = record.data() {
                        new_answer.add_rdata(rdata.clone());
                    }
                }

                for rrsig in rrsigs {
                    new_answer.insert_rrsig(rrsig.clone())
                }

                Arc::new(new_answer)
            })
    }

    fn additional_search(
        &self,
        original_name: &LowerName,
        original_query_type: RecordType,
        next_name: LowerName,
        _search_type: RecordType,
        lookup_options: LookupOptions,
    ) -> Option<Vec<Arc<RecordSet>>> {
        let mut additionals: Vec<Arc<RecordSet>> = vec![];

        // if it's a CNAME or other forwarding record, we'll be adding additional records based on the query_type
        let mut query_types_arr = [original_query_type; 2];
        let query_types: &[RecordType] = match original_query_type {
            RecordType::ANAME | RecordType::NS | RecordType::MX | RecordType::SRV => {
                query_types_arr = [RecordType::A, RecordType::AAAA];
                &query_types_arr[..]
            }
            _ => &query_types_arr[..1],
        };

        for query_type in query_types {
            // loop and collect any additional records to send

            // Track the names we've looked up for this query type.
            let mut names = HashSet::new();

            // If we're just going to repeat the same query then bail out.
            if query_type == &original_query_type {
                names.insert(original_name.clone());
            }

            let mut next_name = Some(next_name.clone());
            while let Some(search) = next_name.take() {
                // If we've already looked up this name then bail out.
                if names.contains(&search) {
                    break;
                }

                let additional = self.inner_lookup(&search, *query_type, lookup_options);
                names.insert(search);

                if let Some(additional) = additional {
                    // assuming no crazy long chains...
                    if !additionals.contains(&additional) {
                        additionals.push(additional.clone());
                    }

                    next_name =
                        maybe_next_name(&additional, *query_type).map(|(name, _search_type)| name);
                }
            }
        }

        if !additionals.is_empty() {
            Some(additionals)
        } else {
            None
        }
    }
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
        if !self.origin.zone_of(name) {
            info!("is not origin.");
            return Err(LookupError::ResponseCode(ResponseCode::NotImp));
        }

        let (result, additionals): (LookupResult<LookupRecords>, Option<LookupRecords>) =
            match rtype {
                RecordType::AXFR | RecordType::ANY => {
                    let inner = &self.inner;
                    let res = inner.inner_lookup(name.borrow())?;

                    let rrset = res
                        .into_iter()
                        .map(|(tp, rdata)| {
                            Arc::new({
                                let mut set = RecordSet::new(name.borrow(), tp, 0);
                                if !set.add_rdata(rdata) {
                                    // TODO:
                                    error!("insert rdata failed.");
                                };
                                set
                            })
                        })
                        .collect();
                    let result = AnyRecords::new(lookup_options, rrset, rtype, name.clone());
                    (Ok(LookupRecords::AnyRecords(result)), None)
                }
                _ => {
                    // perform the lookup
                    let answer = self.inner_lookup(name, rtype, lookup_options);

                    // evaluate any cnames for additional inclusion
                    let additionals_root_chain_type: Option<(_, _)> = answer
                        .as_ref()
                        .and_then(|a| maybe_next_name(a, rtype))
                        .and_then(|(search_name, search_type)| {
                            self.additional_search(
                                name,
                                rtype,
                                search_name,
                                search_type,
                                lookup_options,
                            )
                            .map(|adds| (adds, search_type))
                        });

                    // if the chain started with an ANAME, take the A or AAAA record from the list
                    let (additionals, answer) = match (additionals_root_chain_type, answer, rtype) {
                        (Some((additionals, RecordType::ANAME)), Some(answer), RecordType::A)
                        | (
                            Some((additionals, RecordType::ANAME)),
                            Some(answer),
                            RecordType::AAAA,
                        ) => {
                            // This should always be true...
                            debug_assert_eq!(answer.record_type(), RecordType::ANAME);

                            // in the case of ANAME the final record should be the A or AAAA record
                            let (rdatas, a_aaaa_ttl) = {
                                let last_record = additionals.last();
                                let a_aaaa_ttl = last_record.map_or(u32::max_value(), |r| r.ttl());

                                // grap the rdatas
                                let rdatas: Option<Vec<RData>> = last_record
                                    .and_then(|record| match record.record_type() {
                                        RecordType::A | RecordType::AAAA => {
                                            // the RRSIGS will be useless since we're changing the record type
                                            Some(record.records_without_rrsigs())
                                        }
                                        _ => None,
                                    })
                                    .map(|records| {
                                        records
                                            .filter_map(Record::data)
                                            .cloned()
                                            .collect::<Vec<_>>()
                                    });

                                (rdatas, a_aaaa_ttl)
                            };

                            // now build up a new RecordSet
                            //   the name comes from the ANAME record
                            //   according to the rfc the ttl is from the ANAME
                            //   TODO: technically we should take the min of the potential CNAME chain
                            let ttl = answer.ttl().min(a_aaaa_ttl);
                            let mut new_answer = RecordSet::new(answer.name(), rtype, ttl);

                            for rdata in rdatas.into_iter().flatten() {
                                new_answer.add_rdata(rdata);
                            }

                            // ANAME's are constructed on demand, so need to be signed before return
                            if lookup_options.is_dnssec() {
                                // TODO:
                                // InnerInMemory::sign_rrset(
                                //     &mut new_answer,
                                //     inner.secure_keys(),
                                //     inner.minimum_ttl(self.origin()),
                                //     self.class(),
                                // )
                                // // rather than failing the request, we'll just warn
                                // .map_err(|e| warn!("failed to sign ANAME record: {}", e))
                                // .ok();
                            }

                            // prepend answer to additionals here (answer is the ANAME record)
                            let additionals = std::iter::once(answer)
                                .chain(additionals.into_iter())
                                .collect();

                            // return the new answer
                            //   because the searched set was an Arc, we need to arc too
                            (Some(additionals), Some(Arc::new(new_answer)))
                        }
                        (Some((additionals, _)), answer, _) => (Some(additionals), answer),
                        (None, answer, _) => (None, answer),
                    };

                    // map the answer to a result
                    let answer = answer
                        .map_or(Err(LookupError::from(ResponseCode::NXDomain)), |rr_set| {
                            Ok(LookupRecords::new(lookup_options, rr_set))
                        });

                    let additionals = additionals.map(|a| LookupRecords::many(lookup_options, a));

                    (answer, additionals)
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

        // if this is an AXFR zone transfer, verify that this is either the Secondary or Primary
        //  for AXFR the first and last record must be the SOA
        if RecordType::AXFR == rtype {
            // TODO: support more advanced AXFR options
            if !self.is_axfr_allowed() {
                return Err(LookupError::from(ResponseCode::Refused));
            }

            #[allow(deprecated)]
            match self.zone_type() {
                ZoneType::Primary | ZoneType::Secondary | ZoneType::Master | ZoneType::Slave => (),
                // TODO: Forward?
                _ => return Err(LookupError::from(ResponseCode::NXDomain)),
            }
        }

        // perform the actual lookup
        match rtype {
            RecordType::SOA => self.lookup(self.origin(), rtype, lookup_options).await,
            RecordType::AXFR => {
                // TODO: shouldn't these SOA's be secure? at least the first, perhaps not the last?
                let lookup = future::try_join3(
                    // TODO: maybe switch this to be an soa_inner type call?
                    self.soa_secure(lookup_options),
                    self.soa(),
                    self.lookup(name, rtype, lookup_options),
                )
                .map_ok(|(start_soa, end_soa, records)| match start_soa {
                    l @ AuthLookup::Empty => l,
                    start_soa => AuthLookup::AXFR {
                        start_soa: start_soa.unwrap_records(),
                        records: records.unwrap_records(),
                        end_soa: end_soa.unwrap_records(),
                    },
                });

                lookup.await
            }
            // A standard Lookup path
            _ => self.lookup(name, rtype, lookup_options).await,
        }
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

    let baidu = Name::from_str("www.baidu.com").unwrap();
    let record = RData::CNAME(baidu);
    println!("{}", record.to_record_type());
    let read = bincode::serde::encode_to_vec(record, bincode::config::legacy()).unwrap();
    println!("{:?}", hex::encode(&read));
    let raw_str = String::from_utf8_lossy(&read);
    let baidu_str = "www.baidu.com";
    println!("{:?}", hex::encode(baidu_str.as_bytes()));
    println!("{raw_str}");
    let decode = bincode::serde::decode_from_slice::<RData, _>(&read, bincode::config::legacy())
        .unwrap()
        .0;
    println!("{}", decode.to_record_type())
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

fn is_need_type(key_type: RecordType, query_type: RecordType) -> bool {
    key_type == query_type
        || key_type == RecordType::CNAME
        || (query_type == RecordType::A || query_type == RecordType::AAAA)
            && key_type == RecordType::ANAME
}

/// Gets the next search name, and returns the RecordType that it originated from
fn maybe_next_name(
    record_set: &RecordSet,
    query_type: RecordType,
) -> Option<(LowerName, RecordType)> {
    match (record_set.record_type(), query_type) {
        // ANAME is similar to CNAME,
        //  unlike CNAME, it is only something that continue to additional processing if the
        //  the query was for address (A, AAAA, or ANAME itself) record types.
        (t @ RecordType::ANAME, RecordType::A)
        | (t @ RecordType::ANAME, RecordType::AAAA)
        | (t @ RecordType::ANAME, RecordType::ANAME) => record_set
            .records_without_rrsigs()
            .next()
            .and_then(Record::data)
            .and_then(RData::as_aname)
            .map(LowerName::from)
            .map(|name| (name, t)),
        (t @ RecordType::NS, RecordType::NS) => record_set
            .records_without_rrsigs()
            .next()
            .and_then(Record::data)
            .and_then(RData::as_ns)
            .map(LowerName::from)
            .map(|name| (name, t)),
        // CNAME will continue to additional processing for any query type
        (t @ RecordType::CNAME, _) => record_set
            .records_without_rrsigs()
            .next()
            .and_then(Record::data)
            .and_then(RData::as_cname)
            .map(LowerName::from)
            .map(|name| (name, t)),
        (t @ RecordType::MX, RecordType::MX) => record_set
            .records_without_rrsigs()
            .next()
            .and_then(Record::data)
            .and_then(RData::as_mx)
            .map(|mx| mx.exchange().clone())
            .map(LowerName::from)
            .map(|name| (name, t)),
        (t @ RecordType::SRV, RecordType::SRV) => record_set
            .records_without_rrsigs()
            .next()
            .and_then(Record::data)
            .and_then(RData::as_srv)
            .map(|srv| srv.target().clone())
            .map(LowerName::from)
            .map(|name| (name, t)),
        // other additional collectors can be added here can be added here
        _ => None,
    }
}
