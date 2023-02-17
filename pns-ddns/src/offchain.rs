use pns_resolvers::resolvers::Config;
use pns_types::{ddns::codec_type::RecordType, DomainHash};
use sc_client_api::backend::Backend as BackendT;
use sp_api::{
    offchain::{DbExternalities, OffchainStorage},
    Encode,
};
use tracing::debug;

pub struct OffChain<Storage> {
    pub db: PersistentOffchainDb<Storage>,
}

impl<Storage: OffchainStorage> OffChain<Storage> {
    pub fn get<T: Config>(&mut self, id: DomainHash) -> Vec<(RecordType, Vec<u8>)> {
        self.db.get::<T>(id)
    }

    pub fn set(&mut self, k: &[u8], v: &[u8], _timestamp: i64) {
        // TODO: check timestamp
        self.db.set(k, v);
    }

    pub fn set_with_signature<
        T: Config,
        Checker: Send + Sync + FnOnce(pns_types::DomainHash, &T::AccountId) -> bool,
    >(
        &mut self,
        who: T::AccountId,
        code: T::Signature,
        id: DomainHash,
        tp: RecordType,
        content: Vec<u8>,
        check_node_useable: Checker,
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        debug!(
            "{who:?} will set with signature: {code:?} id: {id:?} tp: {tp:?} content: {content:?}"
        );
        // TODO:
        if check_node_useable(id, &who) {
            let data = (id, tp, &content).encode();
            use sp_runtime::traits::Verify;
            if code.verify(&data[..], &who) {
                let k = DataOperations::offchain_key_with_type::<T>(id, tp);
                self.db.set(&k, &content);

                return Some((k, content));
            }
        }
        None
    }
}

pub struct DataOperations;

impl DataOperations {
    #[inline]
    pub fn offchain_key<T: Config>(id: DomainHash) -> Vec<u8> {
        (<T as Config>::OFFCHAIN_PREFIX, id).encode()
    }

    #[inline]
    pub fn offchain_key_with_type<T: Config>(id: DomainHash, tp: RecordType) -> Vec<u8> {
        let key = (<T as Config>::OFFCHAIN_PREFIX, id).encode();
        (key, tp).encode()
    }
    #[inline]
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
}

pub struct PersistentOffchainDb<Storage> {
    db: sc_offchain::OffchainDb<Storage>,
}

pub fn from_backend<Block: sp_api::BlockT, Backend: BackendT<Block>>(
    backend: &Backend,
) -> Option<PersistentOffchainDb<<Backend as BackendT<Block>>::OffchainStorage>> {
    backend
        .offchain_storage()
        .map(|storage| PersistentOffchainDb {
            db: sc_offchain::OffchainDb::new(storage),
        })
}

impl<Storage: OffchainStorage> PersistentOffchainDb<Storage> {
    pub fn set(&mut self, k: &[u8], v: &[u8]) {
        self.db
            .local_storage_set(sp_api::offchain::StorageKind::PERSISTENT, k, v);
    }

    fn get_raw(&mut self, k: &[u8]) -> Option<Vec<u8>> {
        self.db
            .local_storage_get(sp_api::offchain::StorageKind::PERSISTENT, k)
    }

    pub fn get<T: Config>(&mut self, id: DomainHash) -> Vec<(RecordType, Vec<u8>)> {
        let keys = DataOperations::keys::<T>(id);
        keys.into_iter()
            .filter_map(|(tp, k)| self.get_raw(&k).map(|v| (tp, v)))
            .collect()
    }
}
