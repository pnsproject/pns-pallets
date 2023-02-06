use pns_resolvers::resolvers::Config;
use pns_types::{ddns::codec_type::RecordType, DomainHash};
use redb::{Database, ReadableTable, TableDefinition};
use sp_api::Encode;
use tracing::debug;

const DDNS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("ddns");

const FILE: &str = "ddns.redb";

pub struct OffChain;

impl OffChain {
    pub fn offchain_key<T: Config>(id: DomainHash) -> Vec<u8> {
        (<T as Config>::OFFCHAIN_PREFIX, id).encode()
    }

    pub fn offchain_key_with_type<T: Config>(id: DomainHash, tp: RecordType) -> Vec<u8> {
        let key = (<T as Config>::OFFCHAIN_PREFIX, id).encode();
        (key, tp).encode()
    }

    pub fn keys<T: Config>(id: DomainHash) -> Vec<(RecordType, Vec<u8>)> {
        let key = Self::offchain_key::<T>(id);
        RecordType::all()
            .into_iter()
            .map(|tp| {
                let k = (&key, tp).encode();
                (tp, k)
            })
            .collect()
    }

    pub fn get<T: Config>(id: DomainHash) -> Vec<(RecordType, Vec<u8>)> {
        let keys = Self::keys::<T>(id);
        let db = Database::create(FILE).expect("create database failed.");
        let read_txn = match db.begin_read() {
            Ok(txn) => txn,
            Err(e) => {
                tracing::error!("failed to begin read: {}", e);
                return vec![];
            }
        };
        let table = match read_txn.open_table(DDNS_TABLE) {
            Ok(table) => table,
            Err(e) => {
                tracing::error!("failed to open table: {}", e);
                return vec![];
            }
        };

        keys.into_iter()
            .filter_map(|(tp, k)| {
                let res = table.get(&*k).ok()??;

                Some((tp, res.value().to_vec()))
            })
            .collect()
    }

    pub fn set_with_signature<
        T: Config,
        Checker: FnOnce(pns_types::DomainHash, &T::AccountId) -> bool,
    >(
        who: T::AccountId,
        code: T::Signature,
        id: DomainHash,
        tp: RecordType,
        content: Vec<u8>,
        check_node_useable: Checker,
    ) -> bool {
        debug!(
            "{who:?} will set with signature: {code:?} id: {id:?} tp: {tp:?} content: {content:?}"
        );
        // TODO:

        if check_node_useable(id, &who) {
            let data = (id, tp, &content).encode();
            use sp_runtime::traits::Verify;
            if code.verify(&data[..], &who) {
                let db = Database::create(FILE).expect("create database failed.");
                let Ok(write_txn) = db.begin_write() else {
                    return false;
                };

                {
                    let mut table = match write_txn.open_table(DDNS_TABLE) {
                        Ok(table) => table,
                        Err(e) => {
                            tracing::error!("failed to open table: {:?}", e);
                            return false;
                        }
                    };
                    let k = Self::offchain_key_with_type::<T>(id, tp);

                    if let Err(e) = table.insert(&*k, &*content) {
                        tracing::error!("{e:?}");
                        return false;
                    };
                }
                if let Err(e) = write_txn.commit() {
                    tracing::error!("{e:?}");
                    return false;
                }

                return true;
            }
        }
        false
    }
}
