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
//! - `set_owner` - transfer a domain name, requires the caller to have permission to operate the domain name
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
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// 解析器Id
        type ResolverId: Clone + Decode + Encode + Eq + PartialEq + core::fmt::Debug + Default;
        /// 提供nft层的抽象，用来分配域名
        type Registry: Registry<
            AccountId = Self::AccountId,
            Hash = Self::Hash,
            Balance = BalanceOf<Self>,
        >;
        /// 用来交易
        type Currency: ReservableCurrency<Self::AccountId>;
        /// 表示事件的类型，单位是秒
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
        /// now提供者
        type NowProvider: UnixTime;
        /// 常量，宽限期
        #[pallet::constant]
        type GracePeriod: Get<Self::Moment>;
        /// 常量，默认的子域名容量
        #[pallet::constant]
        type DefaultCapacity: Get<u32>;
        /// 常量，basenode
        #[pallet::constant]
        type BaseNode: Get<Self::Hash>;
        /// 常量最小注册时长
        #[pallet::constant]
        type MinRegistrationDuration: Get<Self::Moment>;

        type WeightInfo: WeightInfo;
        /// 价格预言机接口
        type PriceOracle: PriceOracle<Duration = Self::Moment, Balance = BalanceOf<Self>>;
        /// 管理员权限验证接口
        type ManagerOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;
        /// 是否开启域名分发接口
        type IsOpen: IsRegistrarOpen;
        /// 官方账号接口
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
        /// `[name,node,owner,expire]`
        /// 域名已注册
        NameRegistered(Vec<u8>, T::Hash, T::AccountId, T::Moment),
        // to frontend call
        /// When a domain name is successfully renewed, this moment will be logged.
        /// `[name,node,duration]`
        /// 域名已续费
        NameRenewed(Vec<u8>, T::Hash, T::Moment),
        /// When a sub-domain name is successfully registered, this moment will be logged.
        /// `[label,subnode,owner,node]`
        /// 子域名已注册
        SubnameRegistered(Vec<u8>, T::Hash, T::AccountId, T::Hash),
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
        /// 添加到预留名单
        #[pallet::weight(T::WeightInfo::add_reserved())]
        pub fn add_reserved(origin: OriginFor<T>, node: T::Hash) -> DispatchResult {
            // 验证管理员权限
            let _who = T::ManagerOrigin::ensure_origin(origin)?;
            // 插入该域名到预留名单
            ReservedList::<T>::insert(node, ());
            Ok(())
        }
        /// Remove a domain from the reserved list
        /// Only root
        /// 从预留名单内移除
        #[pallet::weight(T::WeightInfo::remove_reserved())]
        pub fn remove_reserved(origin: OriginFor<T>, node: T::Hash) -> DispatchResult {
            // 验证管理员权限
            let _who = T::ManagerOrigin::ensure_origin(origin)?;
            // 从预留名单移除该域名
            ReservedList::<T>::remove(node);
            Ok(())
        }
        /// Register a domain name.
        ///
        /// Note: The domain name must conform to the rules,
        /// while the interface is only responsible for
        /// registering domain names greater than 10 in length.
        ///
        /// Ensure: The name must be unoccupied.
        /// 注册域名接口
        #[pallet::weight(T::WeightInfo::register(name.len() as u32))]
        #[frame_support::transactional]
        pub fn register(
            // 域名注册的调用者（也是付钱的人，但域名不一定是他的）
            origin: OriginFor<T>,
            // 传入的名字的字节序列
            // 比如要注册cupnfish.dot
            // 前端应该输入cupnfish
            // 而不是完整的cupnfish.dot
            name: Vec<u8>,
            // 注册的域名是给谁的
            owner: <T::Lookup as StaticLookup>::Source,
            // 注册的时长
            duration: T::Moment,
        ) -> DispatchResult {
            // 验证调用者是签名账户
            let caller = ensure_signed(origin)?;
            let owner = T::Lookup::lookup(owner)?;
            // 确保分发中心是打开状态
            ensure!(T::IsOpen::is_open(), Error::<T>::RegistrarClosed);
            // 确保注册时间大于最小注册时间
            ensure!(
                duration >= T::MinRegistrationDuration::get(),
                Error::<T>::RegistryDurationInvalid
            );
            // 从字节码序列解析得到域名label
            let (label, label_len) =
                Label::<T::Hash>::new(&name).ok_or(Error::<T>::ParseLabelFailed)?;

            use crate::traits::Available;
            // 判断长度是否在可注册范围内
            ensure!(label_len.is_registrable(), Error::<T>::LabelInvalid);
            // 获取官方账号
            let official = T::Official::get_official_account()?;
            // 获取当前时间
            let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
            // 计算注册的期限
            let expire = now
                .checked_add(&duration)
                .ok_or(ArithmeticError::Overflow)?;

            // 防止计算结果在未来验证时发生溢出
            ensure!(
                expire + T::GracePeriod::get() > now + T::GracePeriod::get(),
                ArithmeticError::Overflow
            );
            // 获取basenode，并计算label_node
            let base_node = T::BaseNode::get();
            let label_node = label.encode_with_node(&base_node);
            // 确保label_node不在预留名单内
            ensure!(
                !ReservedList::<T>::contains_key(&label_node),
                Error::<T>::Frozen
            );
            // 调用注册接口
            T::Registry::mint_subname(
                &official,
                base_node,
                label_node,
                owner.clone(),
                // 容量输入任何都可以，因为这是一个基于basenode的调用
                // 在mint_subname那边没有校验容量
                0,
                |maybe_pre_owner| -> DispatchResult {
                    // 从价格预言机获取注册费
                    let register_fee = T::PriceOracle::register_fee(label_len, duration)
                        .ok_or(ArithmeticError::Overflow)?;
                    // 从价格预言机获取需要缴纳的押金
                    let deposit =
                        T::PriceOracle::deposit_fee(label_len).ok_or(ArithmeticError::Overflow)?;
                    // 计算总的需要交易的钱
                    let target_value = register_fee
                        .checked_add(&deposit)
                        .ok_or(ArithmeticError::Overflow)?;
                    // 从调用者扣费到官方
                    T::Currency::transfer(
                        &caller,
                        &official,
                        target_value,
                        // allow death即允许死亡，允许交易的账户最小资产低于存在阙值
                        ExistenceRequirement::AllowDeath,
                    )?;
                    // 修改注册信息
                    RegistrarInfos::<T>::mutate(label_node, |info| -> DispatchResult {
                        if let Some(info) = info.as_mut() {
                            // 存在之前的信息时
                            // 如果存在之前的所有者，退还押金给之前的所有者
                            if let Some(pre_owner) = maybe_pre_owner {
                                T::Currency::transfer(
                                    &official,
                                    pre_owner,
                                    info.deposit,
                                    ExistenceRequirement::KeepAlive,
                                )?;
                            }
                            // 更改为当前的押金
                            // 更改为当前的注册费
                            // 更改为当前的期限
                            info.deposit = deposit;
                            info.register_fee = register_fee;
                            info.expire = expire;
                        } else {
                            // 如果此前没有信息
                            // 插入新的信息
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
            // 保存域名注册事件
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
        /// 续费，同样传入的是字节码序列的名字
        #[pallet::weight(T::WeightInfo::renew(name.len() as u32))]
        #[frame_support::transactional]
        pub fn renew(origin: OriginFor<T>, name: Vec<u8>, duration: T::Moment) -> DispatchResult {
            // 调用者需要签名
            let caller = ensure_signed(origin)?;
            // 确保分发中心是开启的
            ensure!(T::IsOpen::is_open(), Error::<T>::RegistrarClosed);
            // 从字节码序列解析为label
            let (label, label_len) =
                Label::<T::Hash>::new(&name).ok_or(Error::<T>::ParseLabelFailed)?;
            // 从label获取相应的域名哈希
            let label_node = label.encode_with_node(&T::BaseNode::get());
            // 修改注册信息
            RegistrarInfos::<T>::mutate(label_node, |info| -> DispatchResult {
                // 信息不存在则报错
                let info = info.as_mut().ok_or(Error::<T>::NotExistOrOccupied)?;
                // 之前存在的期限
                let expire = info.expire;
                // 当前的时间
                let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
                // 宽限续费的期限
                let grace_period = T::GracePeriod::get();
                // 确保在宽限期内
                ensure!(now <= expire + grace_period, Error::<T>::NotRenewable);
                // 计算出最终的期限
                let target_expire = expire
                    .checked_add(&duration)
                    .ok_or(ArithmeticError::Overflow)?;
                // 确保将来计算宽限期时不会溢出
                ensure!(
                    target_expire + grace_period > now + grace_period,
                    ArithmeticError::Overflow
                );
                // 从价格预言机获取续费需要支付的金额
                let price = T::PriceOracle::renew_fee(label_len, duration)
                    .ok_or(ArithmeticError::Overflow)?;
                // 支付金额
                T::Currency::transfer(
                    &caller,
                    &T::Official::get_official_account()?,
                    price,
                    ExistenceRequirement::AllowDeath,
                )?;
                // 更改信息的期限为新的期限
                info.expire = target_expire;
                // 保存续费成功的事件
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
        /// 设置新的所有者，相当于交易域名
        #[pallet::weight(T::WeightInfo::set_owner())]
        #[frame_support::transactional]
        pub fn set_owner(
            origin: OriginFor<T>,
            to: <T::Lookup as StaticLookup>::Source,
            node: T::Hash,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let to = T::Lookup::lookup(to)?;
            // 确保注册中心是开启的
            ensure!(T::IsOpen::is_open(), Error::<T>::RegistrarClosed);

            if let Some(info) = RegistrarInfos::<T>::get(node) {
                // 如果注册信息存在，则判断当前域名是否在可操作事件范围内
                let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
                ensure!(
                    info.expire + T::GracePeriod::get() > now,
                    Error::<T>::NotOwned
                );
            }
            // 将域名交易出去
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
        /// 铸造子域名
        #[pallet::weight(T::WeightInfo::mint_subname(data.len() as u32))]
        #[frame_support::transactional]
        pub fn mint_subname(
            origin: OriginFor<T>,
            node: T::Hash,
            // 同样是子域名的label
            data: Vec<u8>,
            to: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let to = T::Lookup::lookup(to)?;

            ensure!(T::IsOpen::is_open(), Error::<T>::RegistrarClosed);
            // 获取容量
            let capacity = RegistrarInfos::<T>::get(node)
                .map(|info| info.capacity)
                .unwrap_or_else(T::DefaultCapacity::get);

            let (label, _) = Label::new(&data).ok_or(Error::<T>::ParseLabelFailed)?;
            let label_node = label.encode_with_node(&node);
            // 调用铸造子域名的接口
            T::Registry::mint_subname(&caller, node, label_node, to.clone(), capacity, |_| Ok(()))?;
            // 保存子域名铸造成功的事件
            Self::deposit_event(Event::<T>::SubnameRegistered(data, label_node, to, node));

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
    fn set_owner() -> Weight;
    fn add_reserved() -> Weight;
    fn remove_reserved() -> Weight;
}

// 实现分发中心的接口
impl<T: Config> crate::traits::Registrar for Pallet<T> {
    type Hash = T::Hash;
    type Balance = BalanceOf<T>;
    type AccountId = T::AccountId;
    type Duration = T::Moment;
    // 检查是否可注册
    fn check_expires_registrable(node: Self::Hash) -> sp_runtime::DispatchResult {
        // 获取当前的事件
        let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
        // 获取当前的期限
        let expire = RegistrarInfos::<T>::get(node)
            .ok_or(Error::<T>::NotExistOrOccupied)?
            .expire;
        // 当前的时间必须大于存在的期限加上宽限时间，否则该域名仍然是被占用状态
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
    // 检查是否处于可续费阶段
    fn check_expires_renewable(node: Self::Hash) -> sp_runtime::DispatchResult {
        // 获取当前时间
        let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
        // 获取当前期限
        let expire = RegistrarInfos::<T>::get(node)
            .ok_or(Error::<T>::NotExistOrOccupied)?
            .expire;
        // 确保当前时间小于当前期限加上宽限期
        frame_support::ensure!(
            now < expire + T::GracePeriod::get(),
            Error::<T>::NotRenewable
        );

        Ok(())
    }
    // 检车是否处于可用阶段
    fn check_expires_useable(node: Self::Hash) -> sp_runtime::DispatchResult {
        // 获取当前时间
        let now = IntoMoment::<T>::into_moment(T::NowProvider::now());
        // 获取当前期限
        let expire = RegistrarInfos::<T>::get(node)
            .ok_or(Error::<T>::NotExistOrOccupied)?
            .expire;
        // 确保当前时间小于当前期限
        frame_support::ensure!(now < expire, Error::<T>::NotUseable);

        Ok(())
    }
    // 清楚注册信息
    fn clear_registrar_info(
        node: Self::Hash,
        owner: &Self::AccountId,
    ) -> sp_runtime::DispatchResult {
        // 获取官方账号
        let official = T::Official::get_official_account()?;
        RegistrarInfos::<T>::mutate_exists(node, |info| -> Option<()> {
            // 如果存在相应的信息
            if let Some(info) = info {
                // 把押金退还给所有者
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
    // 给兑换码调用的接口
    fn for_redeem_code(
        // 传入的字节序列名称
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
        // 调用铸造子域名接口
        T::Registry::mint_subname(
            &official,
            base_node,
            label_node,
            to.clone(),
            0,
            |maybe_pre_owner| -> DispatchResult {
                RegistrarInfos::<T>::mutate(label_node, |info| -> DispatchResult {
                    if let Some(info) = info.as_mut() {
                        // 如果存在之前的所有者，则退还押金
                        if let Some(pre_owner) = maybe_pre_owner {
                            T::Currency::transfer(
                                &official,
                                pre_owner,
                                info.deposit,
                                frame_support::traits::ExistenceRequirement::KeepAlive,
                            )?;
                        }
                        // 兑换码获得的押金和注册费都是0
                        info.deposit = Zero::zero();
                        info.register_fee = Zero::zero();
                        info.expire = expire;
                    } else {
                        // 没有则创建新的信息
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
        0
    }

    fn register(_len: u32) -> Weight {
        0
    }

    fn renew(_len: u32) -> Weight {
        0
    }

    fn set_owner() -> Weight {
        0
    }

    fn add_reserved() -> Weight {
        0
    }

    fn remove_reserved() -> Weight {
        0
    }
}
