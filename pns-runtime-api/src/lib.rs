#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::{Decode, Encode};
use pns_types::{ddns::codec_type::RecordType, DomainHash, RegistrarInfo};
use sp_runtime::traits::MaybeSerialize;

sp_api::decl_runtime_apis! {
    pub trait PnsStorageApi<Duration, Balance,Signature,AccountId>
    where Duration: Decode + Encode + MaybeSerialize,
    Balance: Decode+ Encode + MaybeSerialize,
    Signature: Decode + Encode + MaybeSerialize,
    AccountId: Decode + Encode + MaybeSerialize,
    {
        fn get_info(id: DomainHash) -> Option<RegistrarInfo<Duration, Balance>>;
        fn all() -> sp_std::vec::Vec<(DomainHash,RegistrarInfo<Duration, Balance>)>;
        fn lookup(id: DomainHash) -> sp_std::vec::Vec<(RecordType, sp_std::vec::Vec<u8>)>;
        fn check_node_useable(node: DomainHash, owner: &AccountId) -> bool;
        // fn set_record(who: AccountId,code: Signature,id: DomainHash,tp: RecordType,content: sp_std::vec::Vec<u8>) -> bool;
    }
}
