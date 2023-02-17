//! # PNS `Origin`
//!
//! This module is responsible for providing administrator account
//! authentication for `pns-pallets`. Considering that `pns-pallets`
//! does not routinely obtain administrator privileges on the chain,
//! this interface is provided to operate and maintain `pns-pallets`.
//!
//! ## Introduction
//!
//! This module provides storage to mark which account is the manager
//!  and to enable or disable `pns` domain registration.
//!
//! ### Module functions
//!
//! - `set_registrar_open` - sets whether to turn on domain registration
//! - `set_origin` - sets the manager (caller must be an manager)
//! - `set_origin_for_root` - sets the manager (caller must be `Root`)

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::WeightInfo;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{EnsureOrigin, Get};
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::StaticLookup;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type Origins<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, ()>;

    #[pallet::storage]
    pub type IsRegistrarOpen<T: Config> = StorageValue<_, bool, ValueQuery, DefaultOpen>;

    pub struct DefaultOpen;

    impl Get<bool> for DefaultOpen {
        fn get() -> bool {
            true
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub origins: sp_std::vec::Vec<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                origins: sp_std::vec::Vec::with_capacity(0),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for origin in self.origins.iter() {
                Origins::<T>::insert(origin, ())
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddedOrigin(T::AccountId),
        RemovedOrigin(T::AccountId),
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_registrar_open())]
        pub fn set_registrar_open(origin: OriginFor<T>, is_open: bool) -> DispatchResult {
            let _who = Self::ensure_origin(origin)?;

            IsRegistrarOpen::<T>::put(is_open);

            Ok(())
        }
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_origin(*approved))]
        pub fn set_origin(
            origin: OriginFor<T>,
            account: <T::Lookup as StaticLookup>::Source,
            approved: bool,
        ) -> DispatchResult {
            let _who = Self::ensure_origin(origin)?;
            let account = T::Lookup::lookup(account)?;

            if approved {
                Origins::<T>::insert(&account, ());
                Self::deposit_event(Event::<T>::AddedOrigin(account));
            } else {
                Origins::<T>::remove(&account);
                Self::deposit_event(Event::<T>::RemovedOrigin(account));
            }

            Ok(())
        }
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_origin_for_root(*approved))]
        pub fn set_origin_for_root(
            origin: OriginFor<T>,
            account: <T::Lookup as StaticLookup>::Source,
            approved: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let account = T::Lookup::lookup(account)?;

            if approved {
                Origins::<T>::insert(&account, ());
                Self::deposit_event(Event::<T>::AddedOrigin(account));
            } else {
                Origins::<T>::remove(&account);
                Self::deposit_event(Event::<T>::RemovedOrigin(account));
            }

            Ok(())
        }
    }
}
use frame_support::{dispatch::Weight, traits::EnsureOrigin};
use frame_system::RawOrigin;

impl<T: Config> EnsureOrigin<T::RuntimeOrigin> for Pallet<T> {
    type Success = T::AccountId;
    fn try_origin(o: T::RuntimeOrigin) -> Result<Self::Success, T::RuntimeOrigin> {
        o.into().and_then(|o| match o {
            RawOrigin::<T::AccountId>::Signed(who) if Origins::<T>::contains_key(&who) => Ok(who),
            r => Err(T::RuntimeOrigin::from(r)),
        })
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn successful_origin() -> T::RuntimeOrigin {
        use codec::Decode;

        if let Some(o) = Origins::<T>::iter_keys().next() {
            return T::RuntimeOrigin::from(RawOrigin::Signed(o));
        }

        let zero_account_id =
            T::AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
                .expect("infinite length input; no invalid inputs for type; qed");

        T::RuntimeOrigin::from(RawOrigin::Signed(zero_account_id))
    }
}

impl<T: Config> crate::traits::IsRegistrarOpen for Pallet<T> {
    fn is_open() -> bool {
        IsRegistrarOpen::<T>::get()
    }
}

pub trait WeightInfo {
    fn set_origin(approved: bool) -> Weight {
        if approved {
            Self::set_origin_true()
        } else {
            Self::set_origin_false()
        }
    }
    fn set_origin_for_root(approved: bool) -> Weight {
        if approved {
            Self::set_origin_for_root_true()
        } else {
            Self::set_origin_for_root_false()
        }
    }
    fn set_registrar_open() -> Weight;
    fn set_origin_true() -> Weight;
    fn set_origin_false() -> Weight;
    fn set_origin_for_root_true() -> Weight;
    fn set_origin_for_root_false() -> Weight;
}

impl WeightInfo for () {
    fn set_registrar_open() -> Weight {
        Weight::zero()
    }

    fn set_origin_true() -> Weight {
        Weight::zero()
    }

    fn set_origin_false() -> Weight {
        Weight::zero()
    }

    fn set_origin_for_root_true() -> Weight {
        Weight::zero()
    }

    fn set_origin_for_root_false() -> Weight {
        Weight::zero()
    }
}
