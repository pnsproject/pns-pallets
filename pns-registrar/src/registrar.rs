pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use sp_std::vec::Vec;

    use crate::traits::{Label, PriceOracle, Registry};
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency, UnixTime},
        Twox64Concat,
    };
    use frame_system::{ensure_signed, pallet_prelude::*};
    use scale_info::TypeInfo;
    use serde::{Deserialize, Serialize};
    use sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, StaticLookup};
    use sp_std::collections::btree_set::BTreeSet;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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
            + MaybeSerializeDeserialize;

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
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// `name_hash` -> Info{ `expire`, `capacity`, `deposity`, `register_fee`}
    #[pallet::storage]
    pub type RegistrarInfos<T: Config> =
        StorageMap<_, Blake2_128Concat, T::Hash, RegistrarInfoOf<T>>;

    /// `name_hash` if in `black_list` -> ()
    #[pallet::storage]
    pub type BlackList<T: Config> = StorageMap<_, Twox64Concat, T::Hash, (), ValueQuery>;

    #[derive(
        Encode, Decode, PartialEq, Eq, RuntimeDebug, Clone, TypeInfo, Deserialize, Serialize,
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
        pub blacklist: BTreeSet<T::Hash>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                infos: Vec::with_capacity(0),
                blacklist: BTreeSet::new(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (node, info) in self.infos.iter() {
                RegistrarInfos::<T>::insert(node, info);
            }

            for node in self.blacklist.iter() {
                BlackList::<T>::insert(node, ());
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// When a domain name is successfully registered, this moment will be logged.
        /// `[name,node,owner,expire]`
        NameRegistered(Vec<u8>, T::Hash, T::AccountId, T::Moment),
        // to frontend call
        /// When a domain name is successfully renewed, this moment will be logged.
        /// `[name,node,duration]`
        NameRenewed(Vec<u8>, T::Hash, T::Moment),
        /// When a sub-domain name is successfully registered, this moment will be logged.
        /// `[label,subnode,owner,node]`
        SubnameRegistered(Vec<u8>, T::Hash, T::AccountId, T::Hash),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// You are not in possession of the term.
        NotOwned,
        /// The node is still occupied and cannot be registered.
        Occupied,
        /// The time used to calculate the expire overflowed.
        TimeOverflow,
        /// You are reclaiming a subdomain or the domain you want to process does not exist.
        NotExist,
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
    }
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a domain name.
        ///
        /// Note: The domain name must conform to the rules,
        /// while the interface is only responsible for
        /// registering domain names greater than 10 in length.
        ///
        /// Ensure: The name must be unoccupied.
        #[pallet::weight(T::WeightInfo::register())]
        #[frame_support::transactional]
        pub fn register(
            origin: OriginFor<T>,
            name: Vec<u8>,
            owner: <T::Lookup as StaticLookup>::Source,
            duration: T::Moment,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let owner = T::Lookup::lookup(owner)?;

            ensure!(
                duration >= T::MinRegistrationDuration::get(),
                Error::<T>::RegistryDurationInvalid
            );

            let (label, label_len) =
                Label::<T::Hash>::new(&name).ok_or_else(|| Error::<T>::ParseLabelFailed)?;

            use crate::traits::Available;

            ensure!(label_len.is_registrable(), Error::<T>::LabelInvalid);

            let price = T::PriceOracle::renew_price(label_len, duration);

            let official = T::Registry::get_official_account();

            let now = IntoMoment::<T>::into_moment(&T::NowProvider::now());

            let expire = now + duration;
            // 防止计算结果溢出
            ensure!(
                expire + T::GracePeriod::get() > now + T::GracePeriod::get(),
                Error::<T>::TimeOverflow
            );
            let base_node = T::BaseNode::get();
            let label_node = label.encode_with_basenode(base_node);

            ensure!(
                !BlackList::<T>::contains_key(&label_node),
                Error::<T>::Frozen
            );

            T::Registry::mint_subname(
                &official,
                base_node,
                label_node,
                owner.clone(),
                0,
                |maybe_pre_owner| -> DispatchResult {
                    let register_fee = T::PriceOracle::register_fee(label_len);
                    let deposit = register_fee / BalanceOf::<T>::from(2_u32);
                    let target_value = price + register_fee + deposit;
                    T::Currency::transfer(
                        &caller,
                        &official,
                        target_value,
                        ExistenceRequirement::KeepAlive,
                    )?;
                    T::Currency::reserve(&official, deposit)?;
                    RegistrarInfos::<T>::mutate(label_node, |info| -> DispatchResult {
                        if let Some(info) = info.as_mut() {
                            if let Some(pre_owner) = maybe_pre_owner {
                                T::Currency::unreserve(&official, info.deposit);
                                T::Currency::transfer(
                                    &official,
                                    &pre_owner,
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

            Self::deposit_event(Event::<T>::NameRegistered(name, label_node, owner, expire));

            Ok(())
        }
        /// Renew a domain name.
        ///
        /// Note: There is no fixed relationship between the caller and the domain,
        ///  so the front-end needs to remind the user of the relationship between
        ///  the domain and that user at renewal time, as it is the caller's responsibility to pay.
        ///
        /// Ensure: Name is within the renewable period.
        #[pallet::weight(T::WeightInfo::renew())]
        #[frame_support::transactional]
        pub fn renew(origin: OriginFor<T>, name: Vec<u8>, duration: T::Moment) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let (label, label_len) =
                Label::<T::Hash>::new(&name).ok_or_else(|| Error::<T>::ParseLabelFailed)?;

            let label_node = label.encode_with_basenode(T::BaseNode::get());

            RegistrarInfos::<T>::mutate(label_node, |info| -> DispatchResult {
                let info = info.as_mut().ok_or_else(|| Error::<T>::NotExist)?;

                let expire = info.expire;
                let now = IntoMoment::<T>::into_moment(&T::NowProvider::now());
                let grace_period = T::GracePeriod::get();
                ensure!(now <= expire + grace_period, Error::<T>::NotRenewable);
                let target_expire = expire + grace_period + duration;
                ensure!(target_expire > now + grace_period, Error::<T>::TimeOverflow);
                let price = T::PriceOracle::renew_price(label_len, duration);
                T::Currency::transfer(
                    &caller,
                    &T::Registry::get_official_account(),
                    price,
                    ExistenceRequirement::KeepAlive,
                )?;
                info.expire = target_expire;
                Self::deposit_event(Event::<T>::NameRenewed(name, label_node, target_expire));
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
        #[pallet::weight(T::WeightInfo::set_owner())]
        #[frame_support::transactional]
        pub fn set_owner(
            origin: OriginFor<T>,
            to: <T::Lookup as StaticLookup>::Source,
            node: T::Hash,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let to = T::Lookup::lookup(to)?;
            if let Some(info) = RegistrarInfos::<T>::get(node) {
                let now = IntoMoment::<T>::into_moment(&T::NowProvider::now());
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
        #[pallet::weight(T::WeightInfo::mint_subname())]
        pub fn mint_subname(
            origin: OriginFor<T>,
            node: T::Hash,
            data: Vec<u8>,
            to: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let to = T::Lookup::lookup(to)?;
            let capacity = RegistrarInfos::<T>::get(node)
                .map(|info| info.capacity)
                .unwrap_or_else(|| T::DefaultCapacity::get());
            let (label, _) = Label::new(&data).ok_or_else(|| Error::<T>::ParseLabelFailed)?;
            let label_node = label.encode_with_node(node);
            T::Registry::mint_subname(&caller, node, label_node, to.clone(), capacity, |_| Ok(()))?;
            Self::deposit_event(Event::<T>::SubnameRegistered(data, label_node, to, node));

            Ok(())
        }
        /// Give your domain name to the official reclaimed and return it to you for a deposit.
        ///
        /// Note: The return deposit is refunded to the caller's account.
        ///
        /// Ensure: Caller has enough permissions and also enough deposit for the given domain.
        #[pallet::weight(T::WeightInfo::reclaimed())]
        #[frame_support::transactional]
        pub fn reclaimed(origin: OriginFor<T>, node: T::Hash) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            ensure!(
                RegistrarInfos::<T>::contains_key(node),
                Error::<T>::NotExist
            );
            T::Registry::reclaimed(&caller, node)?;
            RegistrarInfos::<T>::mutate(node, |info| -> DispatchResult {
                if let Some(info) = info {
                    let official = T::Registry::get_official_account();
                    T::Currency::unreserve(&official, info.deposit);
                    T::Currency::transfer(
                        &official,
                        &caller,
                        info.deposit,
                        ExistenceRequirement::KeepAlive,
                    )?;
                    info.deposit = Zero::zero();
                }
                Ok(())
            })?;

            Ok(())
        }
    }
}

use crate::traits::{IntoMoment, Label, Registry};
use frame_support::{
    dispatch::{DispatchResult, Weight},
    traits::{Currency, Get, UnixTime},
};
use sp_runtime::traits::Zero;
use sp_std::vec::Vec;

pub trait WeightInfo {
    fn mint_subname() -> Weight;
    fn register() -> Weight;
    fn renew() -> Weight;
    fn set_owner() -> Weight;
    fn reclaimed() -> Weight;
}

impl<T: Config> crate::traits::Registrar for Pallet<T> {
    type Hash = T::Hash;
    type Balance = BalanceOf<T>;
    type AccountId = T::AccountId;
    type Duration = T::Moment;

    fn for_redeem_code(
        name: Vec<u8>,
        to: Self::AccountId,
        duration: Self::Duration,
        label: Label<Self::Hash>,
    ) -> DispatchResult {
        let official = T::Registry::get_official_account();
        let now = IntoMoment::<T>::into_moment(&T::NowProvider::now());
        let expire = now + duration;
        // 防止计算结果溢出
        frame_support::ensure!(
            expire + T::GracePeriod::get() > now + T::GracePeriod::get(),
            Error::<T>::TimeOverflow
        );
        let base_node = T::BaseNode::get();
        let label_node = label.encode_with_basenode(base_node);

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
        Self::deposit_event(Event::<T>::NameRegistered(name, label_node, to, expire));

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

    fn clear_registrar_info(
        node: Self::Hash,
        owner: &Self::AccountId,
    ) -> sp_runtime::DispatchResult {
        RegistrarInfos::<T>::mutate_exists(node, |info| -> Option<()> {
            if let Some(info) = info {
                T::Currency::transfer(
                    &T::Registry::get_official_account(),
                    owner,
                    info.deposit,
                    frame_support::traits::ExistenceRequirement::KeepAlive,
                )
                .ok()?;
            }
            None
        });
        Ok(())
    }

    fn check_expires_useable(node: Self::Hash) -> sp_runtime::DispatchResult {
        let now = IntoMoment::<T>::into_moment(&T::NowProvider::now());

        let expire = RegistrarInfos::<T>::get(node)
            .ok_or_else(|| Error::<T>::NotExist)?
            .expire;

        frame_support::ensure!(now < expire, Error::<T>::NotUseable);

        Ok(())
    }

    fn check_expires_registrable(node: Self::Hash) -> sp_runtime::DispatchResult {
        let now = IntoMoment::<T>::into_moment(&T::NowProvider::now());

        let expire = RegistrarInfos::<T>::get(node)
            .ok_or_else(|| Error::<T>::NotExist)?
            .expire;

        frame_support::ensure!(now > expire + T::GracePeriod::get(), Error::<T>::NotOwned);

        Ok(())
    }

    fn check_expires_renewable(node: Self::Hash) -> sp_runtime::DispatchResult {
        let now = IntoMoment::<T>::into_moment(&T::NowProvider::now());

        let expire = RegistrarInfos::<T>::get(node)
            .ok_or_else(|| Error::<T>::NotExist)?
            .expire;

        frame_support::ensure!(
            now < expire + T::GracePeriod::get(),
            Error::<T>::NotRenewable
        );

        Ok(())
    }
}
use sp_runtime::traits::SaturatedConversion;

impl<T: Config> IntoMoment<T> for core::time::Duration {
    type Moment = T::Moment;

    fn into_moment(&self) -> Self::Moment {
        let duration = self.as_secs();
        SaturatedConversion::saturated_from(duration)
    }
}
