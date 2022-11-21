//! # Registrar
//! This module is the registration center for domain names,
//! and it also records some important information about domain name registration:
//!
//! ```rust
//!     pub struct RegistrarInfo<Duration, Balance> {
//!         /// Expiration time
//!         pub expire: Duration,
//!         /// Capacity of subdomains that can be created
//!         pub capacity: u32,
//!         /// Deposit
//!         pub deposit: Balance,
//!         /// Registration fee
//!         pub register_fee: Balance,
//!     }
//! ```
//! ## Introduction
//! Some of the methods in this module involve the transfer of money,
//! so you need to be as careful as possible when reviewing them.
//!
//! ### Module functions
//! - `add_reserved` - adds a pre-reserved domain name (pre-reserved domains cannot be registered), requires manager privileges
//! - `remove_reserved` - removes a reserved domain name, requires manager privileges
//! - `register` - register a domain name
//! - `renew` - renew a domain name, requires caller to have permission to operate the domain
//! - `transfer` - transfer a domain name, requires the caller to have permission to operate the domain name
//! - `mint_subname` - Cast a subdomain, requires the caller to have permission to operate the domain
//!
//! There is a problem with the part about deposits, first review the process of collecting deposits:
//! 1. the deposit is the transaction of the registered domain name to the `PnsOfficial` account
//! 2. the `PnsOfficial` account then saves the deposit through `T::Currency::reserve` so that it cannot be withdrawn.
//! 3. when the domain is `reclaimed` by the user, `PnsOfficial` calls `T::Currency::unreserve` and returns the deposit to the caller. (Or if the domain name expires and is registered by someone else, then the deposit return logic is executed)
//!
//! This part of the function is obviously much more cumbersome,
//! and if the deposit is simply locked to the corresponding user account,
//! it will restrict operations such as domain transfers, as it will be
//! impossible to trace who paid the deposit when the domain is transferred.
//!
//! At the same time, there will be another potential problem:
//! if the deposit is not set properly, the transaction amount
//! will be too low and the transaction will be restricted.

pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use sp_std::vec::Vec;

    use crate::traits::{IsRegistrarOpen, Label, Official, PriceOracle, Registry};
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, EnsureOrigin, ExistenceRequirement, ReservableCurrency, UnixTime},
        Twox64Concat,
    };
    use frame_system::{ensure_signed, pallet_prelude::*};
    use scale_info::TypeInfo;
    use serde::{Deserialize, Serialize};
    use sp_runtime::traits::{
        AtLeast32BitUnsigned, CheckedAdd, MaybeSerializeDeserialize, StaticLookup,
    };
    use sp_runtime::ArithmeticError;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type ResolverId: Clone + Decode + Encode + Eq + PartialEq + core::fmt::Debug + Default;

        type Registry: Registry<
            AccountId = Self::AccountId,
            Hash = Self::Hash,
            Balance = BalanceOf<Self>,
        >;

        type Currency: ReservableCurrency<Self::AccountId>;

        type Moment: Clone
            + Copy
            + Decode
            + Encode
            + Eq
            + PartialEq
            + core::fmt::Debug
            + Default
            + TypeInfo
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + MaxEncodedLen;

        type NowProvider: UnixTime;

        #[pallet::constant]
        type GracePeriod: Get<Self::Moment>;

        #[pallet::constant]
        type DefaultCapacity: Get<u32>;

        #[pallet::constant]
        type BaseNode: Get<Self::Hash>;

        #[pallet::constant]
        type MinRegistrationDuration: Get<Self::Moment>;

        type WeightInfo: WeightInfo;

        type PriceOracle: PriceOracle<Duration = Self::Moment, Balance = BalanceOf<Self>>;

        type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        type IsOpen: IsRegistrarOpen;

        type Official: Official<AccountId = Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// `name_hash` -> Info{ `expire`, `capacity`, `deposity`, `register_fee`}
    #[pallet::storage]
    pub type RegistrarInfos<T: Config> =
        StorageMap<_, Blake2_128Concat, T::Hash, RegistrarInfoOf<T>>;

    /// `name_hash` if in `reserved_list` -> ()
    #[pallet::storage]
    pub type ReservedList<T: Config> = StorageMap<_, Twox64Concat, T::Hash, (), ValueQuery>;

    #[derive(
        Encode,
        Decode,
        PartialEq,
        Eq,
        RuntimeDebug,
        Clone,
        TypeInfo,
        Deserialize,
        Serialize,
        MaxEncodedLen,
    )]
    pub struct RegistrarInfo<Duration, Balance> {
        /// 到期的时间
        pub expire: Duration,
        /// 可创建的子域名容量
        pub capacity: u32,
        /// 押金
        pub deposit: Balance,
        /// 注册费
        pub register_fee: Balance,
    }

    pub type RegistrarInfoOf<T> = RegistrarInfo<<T as Config>::Moment, BalanceOf<T>>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub infos: Vec<(T::Hash, RegistrarInfoOf<T>)>,
        pub reserved_list: sp_std::collections::btree_set::BTreeSet<T::Hash>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                infos: Vec::with_capacity(0),
                reserved_list: sp_std::collections::btree_set::BTreeSet::new(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (node, info) in self.infos.iter() {
                RegistrarInfos::<T>::insert(node, info);
            }

            for node in self.reserved_list.iter() {
                ReservedList::<T>::insert(node, ());
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// When a domain name is successfully registered, this moment will be logged.
        NameRegistered {
            name: Vec<u8>,
            node: T::Hash,
            owner: T::AccountId,
            expire: T::Moment,
        },
        // to frontend call
        /// When a domain name is successfully renewed, this moment will be logged.
        NameRenewed {
            name: Vec<u8>,
            node: T::Hash,
            duration: T::Moment,
            expire: T::Moment,
        },
        /// When a sub-domain name is successfully registered, this moment will be logged.
        SubnameRegistered {
            label: Vec<u8>,
            subnode: T::Hash,
            owner: T::AccountId,
            node: T::Hash,
        },
        /// Reserve a domain name.
        NameReserved { node: T::Hash },
        /// Cancel a reserved domain name.
        NameUnReserved { node: T::Hash },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// You are not in possession of the term.
        NotOwned,
        /// The node is still occupied and cannot be registered.
        Occupied,
        /// You are processing a subdomain or the domain which does not exist.
        /// Or you are registering an occupied subdomain.
        NotExistOrOccupied,
        /// This domain name is temporarily frozen, if you are the authority of the
        /// country (region) or organization, you can contact the official to get
        /// this domain name for you.
        Frozen,
        /// The label you entered is not parsed properly, maybe there are illegal characters in your label.
        ParseLabelFailed,
        /// The length of the label you entered does not correspond to the requirement.
        ///
        /// The length of the label is calculated according to bytes.
        LabelInvalid,
        /// The domain name has exceeded its trial period, please renew or re-register.
        NotUseable,
        /// The domain name has exceeded the renewal period, please re-register.
        NotRenewable,
        /// You want to register in less time than the minimum time we set.
        RegistryDurationInvalid,
        /// Sorry, the registration center is currently closed, please pay attention to the official message and wait for the registration to open.
        RegistrarClosed,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add a domain from the reserved list
        /// Only root
        #[pallet::weight(T::WeightInfo::add_reserved())]
        pub fn add_reserved(origin: OriginFor<T>, node: T::Hash) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            ReservedList::<T>::insert(node, ());

            Self::deposit_event(Event::<T>::NameReserved { node });
            Ok(())
        }
        /// Remove a domain from the reserved list
        /// Only root
        #[pallet::weight(T::WeightInfo::remove_reserved())]
        pub fn remove_reserved(origin: OriginFor<T>, node: T::Hash) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            ReservedList::<T>::remove(node);

            Self::deposit_event(Event::<T>::NameUnReserved { node });
            Ok(())
        }
        /// Register a domain name.
        ///
        /// Note: The domain name must conform to the rules,
        /// while the interface is only responsible for
        /// registering domain names greater than 10 in length.
        ///
        /// Ensure: The name must be unoccupied.
        #[pallet::weight(T::WeightInfo::register(name.len() as u32))]
        #[frame_support::transactional]
        pub fn register(
            origin: OriginFor<T>,
            name: Vec<u8>,
            owner: <T::Lookup as StaticLookup>::Source,
            duration: T::Moment,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let owner = T::Lookup::lookup(owner)?;

            ensure!(T::IsOpen::is_open(), Error::<T>::RegistrarClosed);

            ensure!(
                duration >= T::MinRegistrationDuration::get(),
                Error::<T>::RegistryDurationInvalid
            );

            let (label, label_len) =
                Label::<T::Hash>::new(&name).ok_or(Error::<T>::ParseLabelFailed)?;

            use crate::traits::Available;

            ensure!(label_len.is_registrable(), Error::<T>::LabelInvalid);

            let official = T::Official::get_official_account()?;

            let now = IntoMoment::<T>::into_moment(T::NowProvider::now());

            let expire = now
                .checked_add(&duration)
                .ok_or(ArithmeticError::Overflow)?;

            // 防止计算结果溢出
            ensure!(
                expire + T::GracePeriod::get() > now + T::GracePeriod::get(),
                ArithmeticError::Overflow
            );
            let base_node = T::BaseNode::get();
            let label_node = label.encode_with_node(&base_node);

            ensure!(
                !ReservedList::<T>::contains_key(label_node),
                Error::<T>::Frozen
            );

            T::Registry::mint_subname(
                &official,
                base_node,
                label_node,
                owner.clone(),
                0,
                |maybe_pre_owner| -> DispatchResult {
                    let register_fee = T::PriceOracle::register_fee(label_len, duration)
                        .ok_or(ArithmeticError::Overflow)?;
                    let deposit =
                        T::PriceOracle::deposit_fee(label_len).ok_or(ArithmeticError::Overflow)?;
                    let target_value = register_fee
                        .checked_add(&deposit)
                        .ok_or(ArithmeticError::Overflow)?;

                    T::Currency::transfer(
                        &caller,
                        &official,
                        target_value,
                        ExistenceRequirement::KeepAlive,
                    )?;
                    RegistrarInfos::<T>::mutate(label_node, |info| -> DispatchResult {
                        if let Some(info) = info.as_mut() {
                            if let Some(pre_owner) = maybe_pre_owner {
                                T::Currency::transfer(
                                    &official,
                                    pre_owner,
                                    info.deposit,
                                    ExistenceRequirement::KeepAlive,
                                )?;
                            }
                            info.deposit = deposit;
                            info.register_fee = register_fee;
                            info.expire = expire;
                        } else {
                            let _ = info.insert(RegistrarInfoOf::<T> {
                                deposit,
                                register_fee,
                                expire,
                                capacity: T::DefaultCapacity::get(),
                            });
                        }
                        Ok(())
                    })?;
                    Ok(())
                },
            )?;

            Self::deposit_event(Event::<T>::NameRegistered {
                name,
                node: label_node,
                owner,
                expire,
            });

            Ok(())
        }
        /// Renew a domain name.
        ///
        /// Note: There is no fixed relationship between the caller and the domain,
        ///  so the front-end needs to remind the user of the relationship between
        ///  the domain and that user at renewal time, as it is the caller's responsibility to pay.
        ///
        /// Ensure: Name is within the renewable period.
        #[pallet::weight(T::WeightInfo::renew(name.len() as u32))]
        #[frame_support::transactional]
        pub fn renew(origin: OriginFor<T>, name: Vec<u8>, duration: T::Moment) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            ensure!(T::IsOpen::is_open(), Error::<T>::RegistrarClosed);

            let (label, label_len) =
                Label::<T::Hash>::new(&name).ok_or(Error::<T>::ParseLabelFailed)?;

            let label_node = label.encode_with_node(&T::BaseNode::get());

            RegistrarInfos::<T>::mutate(label_node, |info| -> DispatchResult {
                let info = info.as_mut().ok_or(Error::<T>::NotExistOrOccupied)?;

                let expire = info.expire;
                let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
                let grace_period = T::GracePeriod::get();
                ensure!(now <= expire + grace_period, Error::<T>::NotRenewable);
                let target_expire = expire
                    .checked_add(&duration)
                    .ok_or(ArithmeticError::Overflow)?;
                ensure!(
                    target_expire + grace_period > now + grace_period,
                    ArithmeticError::Overflow
                );
                let price = T::PriceOracle::renew_fee(label_len, duration)
                    .ok_or(ArithmeticError::Overflow)?;
                T::Currency::transfer(
                    &caller,
                    &T::Official::get_official_account()?,
                    price,
                    ExistenceRequirement::KeepAlive,
                )?;
                info.expire = target_expire;
                Self::deposit_event(Event::<T>::NameRenewed {
                    name,
                    node: label_node,
                    duration,
                    expire: target_expire,
                });
                Ok(())
            })
        }
        /// Trade out your domain name, the caller can be operates.
        ///
        /// Note: Before you trade out your domain name,
        /// you need to note that the deposit you made when registering
        ///  the domain name belongs to the `owner` (or the `operators` of the `owner`)
        ///  of the domain name only,
        /// i.e. the deposit of the domain name traded out will have nothing to do with you.
        ///
        /// Ensure: The front-end should remind the user of the notes.
        #[pallet::weight(T::WeightInfo::transfer())]
        #[frame_support::transactional]
        pub fn transfer(
            origin: OriginFor<T>,
            to: <T::Lookup as StaticLookup>::Source,
            node: T::Hash,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let to = T::Lookup::lookup(to)?;

            ensure!(T::IsOpen::is_open(), Error::<T>::RegistrarClosed);

            if let Some(info) = RegistrarInfos::<T>::get(node) {
                let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
                ensure!(
                    info.expire + T::GracePeriod::get() > now,
                    Error::<T>::NotOwned
                );
            }
            T::Registry::transfer(&who, &to, node)?;
            Ok(())
        }
        /// Create a subdomain.
        ///
        /// Note: The total number of subdomains you can create is certain,
        /// and the subdomains created by your subdomains will take up a
        /// quota of your total subdomains.
        ///
        /// Ensure: The subdomain capacity is sufficient for use.
        #[pallet::weight(T::WeightInfo::mint_subname(data.len() as u32))]
        #[frame_support::transactional]
        pub fn mint_subname(
            origin: OriginFor<T>,
            node: T::Hash,
            data: Vec<u8>,
            to: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let to = T::Lookup::lookup(to)?;

            ensure!(T::IsOpen::is_open(), Error::<T>::RegistrarClosed);

            let capacity = RegistrarInfos::<T>::get(node)
                .map(|info| info.capacity)
                .unwrap_or_else(T::DefaultCapacity::get);
            let (label, _) = Label::new(&data).ok_or(Error::<T>::ParseLabelFailed)?;
            let label_node = label.encode_with_node(&node);
            T::Registry::mint_subname(&caller, node, label_node, to.clone(), capacity, |_| Ok(()))?;
            Self::deposit_event(Event::<T>::SubnameRegistered {
                label: data,
                subnode: label_node,
                owner: to,
                node,
            });

            Ok(())
        }
    }
}

use crate::traits::{IntoMoment, Label, Official, Registry};
use frame_support::{
    dispatch::{DispatchResult, Weight},
    traits::{Currency, Get, UnixTime},
};
use sp_runtime::{
    traits::{CheckedAdd, Zero},
    ArithmeticError,
};
use sp_std::vec::Vec;

pub trait WeightInfo {
    fn mint_subname(len: u32) -> Weight;
    fn register(len: u32) -> Weight;
    fn renew(len: u32) -> Weight;
    fn transfer() -> Weight;
    fn add_reserved() -> Weight;
    fn remove_reserved() -> Weight;
}

impl<T: Config> crate::traits::Registrar for Pallet<T> {
    type Hash = T::Hash;
    type Balance = BalanceOf<T>;
    type AccountId = T::AccountId;
    type Duration = T::Moment;

    fn check_expires_registrable(node: Self::Hash) -> sp_runtime::DispatchResult {
        let now = IntoMoment::<T>::into_moment(T::NowProvider::now());

        let expire = RegistrarInfos::<T>::get(node)
            .ok_or(Error::<T>::NotExistOrOccupied)?
            .expire;

        frame_support::ensure!(now > expire + T::GracePeriod::get(), Error::<T>::Occupied);

        Ok(())
    }

    // fn for_auction_set_expires(
    // 	node: Self::Hash,
    // 	deposit: Self::Balance,
    // 	register_fee: Self::Balance,
    // ) {
    // RegistrarInfos::<T>::mutate(node, |info| {
    // 	let info = info.get_or_insert(RegistrarInfoOf::<T> {
    // 		expire: Default::default(),
    // 		capacity: T::DefaultCapacity::get(),
    // 		deposit: ,
    // 		register_fee:,
    // 	});
    // 	info.expire = frame_system::Pallet::<T>::block_number();

    // 	info.deposit = deposit;
    // 	info.register_fee = register_fee;
    // })
    // }

    fn check_expires_renewable(node: Self::Hash) -> sp_runtime::DispatchResult {
        let now = IntoMoment::<T>::into_moment(T::NowProvider::now());

        let expire = RegistrarInfos::<T>::get(node)
            .ok_or(Error::<T>::NotExistOrOccupied)?
            .expire;

        frame_support::ensure!(
            now < expire + T::GracePeriod::get(),
            Error::<T>::NotRenewable
        );

        Ok(())
    }

    fn check_expires_useable(node: Self::Hash) -> sp_runtime::DispatchResult {
        let now = IntoMoment::<T>::into_moment(T::NowProvider::now());

        let expire = RegistrarInfos::<T>::get(node)
            .ok_or(Error::<T>::NotExistOrOccupied)?
            .expire;

        frame_support::ensure!(now < expire, Error::<T>::NotUseable);

        Ok(())
    }

    fn clear_registrar_info(
        node: Self::Hash,
        owner: &Self::AccountId,
    ) -> sp_runtime::DispatchResult {
        let official = T::Official::get_official_account()?;
        RegistrarInfos::<T>::mutate_exists(node, |info| -> Option<()> {
            if let Some(info) = info {
                T::Currency::transfer(
                    &official,
                    owner,
                    info.deposit,
                    frame_support::traits::ExistenceRequirement::AllowDeath,
                )
                .ok()?;
            }
            None
        });
        Ok(())
    }

    fn for_redeem_code(
        name: Vec<u8>,
        to: Self::AccountId,
        duration: Self::Duration,
        label: Label<Self::Hash>,
    ) -> DispatchResult {
        let official = T::Official::get_official_account()?;
        let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
        let expire = now
            .checked_add(&duration)
            .ok_or(ArithmeticError::Overflow)?;
        // 防止计算结果溢出
        frame_support::ensure!(
            expire + T::GracePeriod::get() > now + T::GracePeriod::get(),
            ArithmeticError::Overflow
        );
        let base_node = T::BaseNode::get();
        let label_node = label.encode_with_node(&base_node);

        T::Registry::mint_subname(
            &official,
            base_node,
            label_node,
            to.clone(),
            0,
            |maybe_pre_owner| -> DispatchResult {
                RegistrarInfos::<T>::mutate(label_node, |info| -> DispatchResult {
                    if let Some(info) = info.as_mut() {
                        if let Some(pre_owner) = maybe_pre_owner {
                            T::Currency::transfer(
                                &official,
                                pre_owner,
                                info.deposit,
                                frame_support::traits::ExistenceRequirement::KeepAlive,
                            )?;
                        }
                        info.deposit = Zero::zero();
                        info.register_fee = Zero::zero();
                        info.expire = expire;
                    } else {
                        let _ = info.insert(RegistrarInfoOf::<T> {
                            deposit: Zero::zero(),
                            register_fee: Zero::zero(),
                            expire,
                            capacity: T::DefaultCapacity::get(),
                        });
                    }
                    Ok(())
                })?;
                Ok(())
            },
        )?;
        Self::deposit_event(Event::<T>::NameRegistered {
            name,
            node: label_node,
            owner: to,
            expire,
        });

        Ok(())
    }

    fn basenode() -> Self::Hash {
        T::BaseNode::get()
    }
}
use sp_runtime::traits::SaturatedConversion;

impl<T: Config> IntoMoment<T> for core::time::Duration {
    type Moment = T::Moment;

    fn into_moment(self) -> Self::Moment {
        let duration = self.as_secs();
        SaturatedConversion::saturated_from(duration)
    }
}

impl WeightInfo for () {
    fn mint_subname(_len: u32) -> Weight {
        Weight::zero()
    }

    fn register(_len: u32) -> Weight {
        Weight::zero()
    }

    fn renew(_len: u32) -> Weight {
        Weight::zero()
    }

    fn transfer() -> Weight {
        Weight::zero()
    }

    fn add_reserved() -> Weight {
        Weight::zero()
    }

    fn remove_reserved() -> Weight {
        Weight::zero()
    }
}
