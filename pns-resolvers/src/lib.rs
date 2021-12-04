#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;

mod traits;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use serde::{Deserialize, Serialize};
    use sp_runtime::{
        traits::{AtLeast32BitUnsigned, CheckEqual, MaybeDisplay, MaybeMallocSizeOf, SimpleBitOps},
        MultiAddress,
    };
    use sp_std::vec;
    use sp_std::vec::Vec;

    use traits::RegistryChecker;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type WeightInfo: WeightInfo;

        type AccountIndex: Parameter + Member + AtLeast32BitUnsigned + Default + Copy;

        type RegistryChecker: RegistryChecker<Hash = Self::DomainHash, AccountId = Self::AccountId>;

        type DomainHash: Parameter
            + Member
            + MaybeSerializeDeserialize
            + sp_std::fmt::Debug
            + MaybeDisplay
            + SimpleBitOps
            + Ord
            + Default
            + Copy
            + CheckEqual
            + sp_std::hash::Hash
            + AsRef<[u8]>
            + AsMut<[u8]>
            + MaybeMallocSizeOf
            + MaxEncodedLen;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
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
    pub enum AddressKind {
        Substrate,
        Bitcoin,
        Ethereum,
    }
    /// account_id mapping
    #[pallet::storage]
    pub type Accounts<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::DomainHash,
        Twox64Concat,
        AddressKind,
        MultiAddress<T::AccountId, T::AccountIndex>,
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
    }
    /// text mapping
    #[pallet::storage]
    pub type Texts<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::DomainHash,
        Twox64Concat,
        TextKind,
        Vec<u8>,
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// [`node`,`address_kind`,`address`]
        pub accounts: Vec<(T::DomainHash, AddressKind, LocalMultiAddress<T::AccountId>)>,
        /// [`node`,`text_kind`,`text`]
        pub texts: Vec<(T::DomainHash, TextKind, Vec<u8>)>,
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
            for (node, address_kind, account) in self.accounts.iter().cloned() {
                let account = match account {
                    LocalMultiAddress::Id(id) => MultiAddress::Id(id),
                    LocalMultiAddress::Raw(raw) => MultiAddress::Raw(raw),
                    LocalMultiAddress::Address32(data) => MultiAddress::Address32(data),
                    LocalMultiAddress::Address20(data) => MultiAddress::Address20(data),
                };
                Accounts::<T>::insert(node, address_kind, account);
            }

            for (node, text_kind, text) in self.texts.iter().cloned() {
                Texts::<T>::insert(node, text_kind, text);
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// [`node`,`kind`,`value`]
        AddressChanged(
            T::DomainHash,
            AddressKind,
            MultiAddress<T::AccountId, T::AccountIndex>,
        ),
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
        #[pallet::weight(T::WeightInfo::set_account())]
        pub fn set_account(
            origin: OriginFor<T>,
            node: T::DomainHash,
            kind: AddressKind,
            address: MultiAddress<T::AccountId, T::AccountIndex>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                T::RegistryChecker::check_node_useable(node, &who),
                Error::<T>::InvalidPermission
            );

            match address {
                MultiAddress::Id(_) => ensure!(
                    kind == AddressKind::Substrate,
                    Error::<T>::ParseAddressFailed
                ),
                MultiAddress::Address32(_) => ensure!(
                    kind == AddressKind::Substrate,
                    Error::<T>::ParseAddressFailed
                ),
                MultiAddress::Address20(_) => ensure!(
                    kind == AddressKind::Ethereum,
                    Error::<T>::ParseAddressFailed
                ),
                MultiAddress::Index(_) => return Err(Error::<T>::NotSupportedIndex.into()),
                _ => {}
            }

            Accounts::<T>::insert(node, kind.clone(), address.clone());

            Self::deposit_event(Event::<T>::AddressChanged(node, kind, address));

            Ok(())
        }
        #[pallet::weight(T::WeightInfo::set_text(content.len()))]
        pub fn set_text(
            origin: OriginFor<T>,
            node: T::DomainHash,
            kind: TextKind,
            content: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                T::RegistryChecker::check_node_useable(node, &who),
                Error::<T>::InvalidPermission
            );

            Texts::<T>::insert(node, kind, content);

            Ok(())
        }
    }
}

use frame_support::dispatch::Weight;

pub trait WeightInfo {
    fn set_text(content_len: usize) -> Weight;
    fn set_account() -> Weight;
}

#[derive(
    codec::Encode,
    codec::Decode,
    PartialEq,
    Eq,
    Clone,
    frame_support::RuntimeDebug,
    scale_info::TypeInfo,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "std", derive(Hash))]
pub enum LocalMultiAddress<AccountId> {
    /// It's an account ID (pubkey).
    Id(AccountId),
    /// It's some arbitrary raw bytes.
    Raw(sp_std::vec::Vec<u8>),
    /// It's a 32 byte representation.
    Address32([u8; 32]),
    /// Its a 20 byte representation.
    Address20([u8; 20]),
}
