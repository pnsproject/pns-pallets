/*!
# Resolvers
This module provides functionality for domain name resolution. Most of these interfaces are interfaces provided for subsequent cooperation with wallets.

### Module functions
- `set_account` - sets the account resolve, which requires the domain to be available relative to that user (ownership of the domain, the domain is not expired)
- `set_text` - set text parsing, same requirements as above
!*/

use codec::{Encode, MaxEncodedLen};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use codec::EncodeLike;
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use pns_types::ddns::codec_type::RecordType;
    use scale_info::TypeInfo;
    use serde::{Deserialize, Serialize};
    use sp_runtime::traits::AtLeast32BitUnsigned;
    use sp_std::vec;

    use super::RegistryChecker;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        const OFFCHAIN_PREFIX: &'static [u8];

        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WeightInfo: WeightInfo;

        type AccountIndex: Parameter + Member + AtLeast32BitUnsigned + Default + Copy;

        type RegistryChecker: RegistryChecker<AccountId = Self::AccountId>;

        type Public: TypeInfo
            + Decode
            + Encode
            + EncodeLike
            + MaybeSerializeDeserialize
            + core::fmt::Debug
            + sp_runtime::traits::IdentifyAccount<AccountId = Self::AccountId>;

        type Signature: sp_runtime::traits::Verify<Signer = Self::Public>
            + codec::Codec
            + EncodeLike
            + MaybeSerializeDeserialize
            + Clone
            + Eq
            + core::fmt::Debug
            + TypeInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[derive(
        Encode,
        Decode,
        Clone,
        Eq,
        PartialEq,
        MaxEncodedLen,
        RuntimeDebug,
        TypeInfo,
        Serialize,
        Deserialize,
    )]
    pub enum Address<Id> {
        Substrate([u8; 32]),
        Bitcoin([u8; 25]),
        Ethereum([u8; 20]),
        Id(Id),
    }
    /// account_id mapping
    #[pallet::storage]
    pub type Accounts<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        pns_types::DomainHash,
        Twox64Concat,
        Address<T::AccountId>,
        (),
    >;
    #[derive(
        Encode,
        Decode,
        Clone,
        Eq,
        PartialEq,
        MaxEncodedLen,
        RuntimeDebug,
        TypeInfo,
        Serialize,
        Deserialize,
    )]
    pub enum TextKind {
        Email,
        Url,
        Avatar,
        Description,
        Notice,
        Keywords,
        Twitter,
        Github,
        Ipfs,
    }
    /// text mapping
    #[pallet::storage]
    pub type Texts<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        pns_types::DomainHash,
        Twox64Concat,
        TextKind,
        Content,
        ValueQuery,
    >;

    /// ddns record
    #[pallet::storage]
    pub type Records<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        pns_types::DomainHash,
        Twox64Concat,
        pns_types::ddns::codec_type::RecordType,
        Content,
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// vec![ `node` , `address` ]
        pub accounts: Vec<(pns_types::DomainHash, Address<T::AccountId>)>,
        /// vec![ `node` , `text_kind` , `text` ]
        pub texts: Vec<(pns_types::DomainHash, TextKind, Content)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                accounts: vec![],
                texts: vec![],
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (node, address_kind) in self.accounts.iter().cloned() {
                Accounts::<T>::insert(node, address_kind, ());
            }

            for (node, text_kind, text) in self.texts.iter().cloned() {
                Texts::<T>::insert(node, text_kind, text);
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddressChanged {
            node: pns_types::DomainHash,
            address: Address<T::AccountId>,
        },
        TextsChanged {
            node: pns_types::DomainHash,
            kind: TextKind,
            content: Content,
        },
        RecordsChanged {
            node: pns_types::DomainHash,
            kind: RecordType,
            content: Content,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Address format parsing failed, are you sure the address and type you entered match?
        ParseAddressFailed,
        /// You do not have enough privileges to change this parameter.
        InvalidPermission,
        /// Not supported address index.
        NotSupportedIndex,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_account())]
        pub fn set_account(
            origin: OriginFor<T>,
            node: pns_types::DomainHash,
            address: Address<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                T::RegistryChecker::check_node_useable(node, &who),
                Error::<T>::InvalidPermission
            );

            Accounts::<T>::insert(node, &address, ());

            Self::deposit_event(Event::<T>::AddressChanged { node, address });

            Ok(())
        }
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_record(content.0.len() as u32))]
        pub fn set_record(
            origin: OriginFor<T>,
            node: pns_types::DomainHash,
            record_type: RecordType,
            content: Content,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                T::RegistryChecker::check_node_useable(node, &who),
                Error::<T>::InvalidPermission
            );

            Records::<T>::insert(node, &record_type, &content);

            Self::deposit_event(Event::<T>::RecordsChanged {
                node,
                kind: record_type,
                content,
            });

            Ok(())
        }
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_text(content.0.len() as u32))]
        pub fn set_text(
            origin: OriginFor<T>,
            node: pns_types::DomainHash,
            kind: TextKind,
            content: Content,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                T::RegistryChecker::check_node_useable(node, &who),
                Error::<T>::InvalidPermission
            );

            Texts::<T>::insert(node, &kind, &content);

            Self::deposit_event(Event::<T>::TextsChanged {
                node,
                kind,
                content,
            });

            Ok(())
        }
    }
}

use frame_support::dispatch::Weight;
use pns_types::{ddns::codec_type::RecordType, DomainHash};
use sp_std::vec::Vec;

pub trait WeightInfo {
    fn set_text(content_len: u32) -> Weight;

    fn set_record(content_len: u32) -> Weight;

    fn set_account() -> Weight;
}

pub trait RegistryChecker {
    type AccountId;
    fn check_node_useable(node: pns_types::DomainHash, owner: &Self::AccountId) -> bool;
}

#[derive(
    codec::Encode,
    codec::Decode,
    PartialEq,
    Eq,
    Clone,
    frame_support::RuntimeDebug,
    scale_info::TypeInfo,
    Default,
)]
#[cfg_attr(feature = "std", derive(Hash, serde::Serialize, serde::Deserialize))]
pub struct Content(pub Vec<u8>);

impl MaxEncodedLen for Content {
    fn max_encoded_len() -> usize {
        1024
    }
}

impl From<Vec<u8>> for Content {
    fn from(inner: Vec<u8>) -> Self {
        Content(inner)
    }
}

impl WeightInfo for () {
    fn set_text(_content_len: u32) -> Weight {
        Weight::zero()
    }

    fn set_record(_content_len: u32) -> Weight {
        Weight::zero()
    }

    fn set_account() -> Weight {
        Weight::zero()
    }
}

impl<C: Config> Pallet<C> {
    pub fn lookup(id: DomainHash) -> Vec<(RecordType, Vec<u8>)> {
        Records::<C>::iter_prefix(id)
            .map(|(k2, v)| (k2, v.0))
            .collect::<Vec<(RecordType, Vec<u8>)>>()
    }
}
