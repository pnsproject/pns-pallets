#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::{Decode, Encode};
use pns_types::{DomainHash, RegistrarInfo};
use sp_runtime::traits::MaybeSerialize;

sp_api::decl_runtime_apis! {
    pub trait PnsStorageApi<Duration, Balance>
    where Duration: Decode + Encode + MaybeSerialize,
    Balance: Decode+ Encode + MaybeSerialize,
    {
        fn get_info(id: DomainHash) -> Option<RegistrarInfo<Duration, Balance>>;
        fn all() -> sp_std::vec::Vec<(DomainHash,RegistrarInfo<Duration, Balance>)>;
    }
}
