//! # Registry
//!
//! This module is a high-level abstraction of the NFT module,
//! and provides `PnsOfficial` storage.
//!
//!
//! ## Introduction
//!
//! Most of the methods of this module are abstracted to higher-level
//! domain name distribution calls (pns-registrar, pns-auction......).
//! But there are still some methods for domain authority management.
//!
//! ### Module functions
//!
//! - `approval_for_all` - share the permissions of all your domains to other accounts
//! - `set_resolver` - set the resolver address of a domain name, which requires permission to operate that domain
//! - `destroy` - destroy a domain, return it to the owner if there is a deposit, requires the domain's operational privileges
//! - `set_official` - Set official account, needs manager privileges
//! - `approve` - share the permission of a domain to another account, requires the permission of the domain

pub use pallet::*;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::{nft, traits::Registrar};
    use codec::FullCodec;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::EnsureOrigin;
    use frame_system::{ensure_signed, pallet_prelude::*};
    use scale_info::TypeInfo;
    use serde::{Deserialize, Serialize};
    use sp_runtime::traits::{StaticLookup, Zero};

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + crate::nft::Config<
            ClassData = (),
            TokenData = Record,
            TokenId = <Self as frame_system::Config>::Hash,
        >
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type WeightInfo: WeightInfo;

        type Registrar: Registrar<Hash = Self::Hash, AccountId = Self::AccountId>;

        type ResolverId: Encode
            + Decode
            + PartialEq
            + Eq
            + core::fmt::Debug
            + Clone
            + Default
            + FullCodec
            + TypeInfo
            + MaxEncodedLen;

        type ManagerOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// 域名记录
    #[derive(
        Encode,
        Decode,
        PartialEq,
        Eq,
        RuntimeDebug,
        Clone,
        Default,
        TypeInfo,
        Serialize,
        Deserialize,
    )]
    pub struct Record {
        pub children: u32,
    }

    /// `name_hash` -> (`origin`,`parent`) or `origin`
    #[pallet::storage]
    pub type Origin<T: Config> = StorageMap<_, Twox64Concat, T::Hash, DomainTracing<T::Hash>>;
    /// `name_hash` -> `resolver_id`
    #[pallet::storage]
    pub type Resolver<T: Config> = StorageMap<_, Twox64Concat, T::Hash, T::ResolverId, ValueQuery>;
    /// `official`
    #[pallet::storage]
    pub type Official<T: Config> = StorageValue<_, T::AccountId>;

    #[derive(
        Encode,
        Decode,
        PartialEq,
        Eq,
        RuntimeDebug,
        Clone,
        TypeInfo,
        Serialize,
        Deserialize,
        MaxEncodedLen,
    )]
    pub enum DomainTracing<Hash> {
        Origin(Hash),
        Root,
    }
    /// (`owner`,`account`) if `account` is `operater` -> ()
    #[pallet::storage]
    pub type OperatorApprovals<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, T::AccountId, (), ValueQuery>;

    /// (`node`,`account`) `node` -> `account`
    #[pallet::storage]
    pub type TokenApprovals<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::Hash, Twox64Concat, T::AccountId, (), ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub origin: Vec<(T::Hash, DomainTracing<T::Hash>)>,
        pub official: Option<T::AccountId>,
        pub operators: Vec<(T::AccountId, T::AccountId)>,
        pub token_approvals: Vec<(T::Hash, T::AccountId)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                origin: Vec::with_capacity(0),
                official: None,
                operators: Vec::with_capacity(0),
                token_approvals: Vec::with_capacity(0),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (node, origin) in self.origin.iter() {
                Origin::<T>::insert(node, origin);
            }
            if let Some(official) = &self.official {
                Official::<T>::put(official);
            }
            for (owner, operator) in self.operators.iter() {
                OperatorApprovals::<T>::insert(owner, operator, ());
            }
            for (hash, to) in self.token_approvals.iter() {
                TokenApprovals::<T>::insert(hash, to, ());
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Logged when the owner of a node assigns a new owner to a subnode.
        /// `[node,owner]`
        NewOwner(T::Hash, T::AccountId),
        /// Logged when the resolver for a node changes.
        /// `[node,resolver]`
        NewResolver(T::Hash, T::ResolverId),
        /// Logged when an operator is added or removed.
        /// `[owner,operator,approved]`
        ApprovalForAll(T::AccountId, T::AccountId, bool),
        /// Logged when a node is traded.
        /// `[from,to,class_id,token_id]`
        Transferred(T::AccountId, T::AccountId, T::ClassId, T::TokenId),
        /// Logged when a node is minted.
        /// `[class_id,token_id,node,owner]`
        TokenMinted(T::ClassId, T::TokenId, T::Hash, T::AccountId),
        /// Logged when a node is burned.
        /// `[class_id,token_id,node,owner,caller]`
        TokenBurned(T::ClassId, T::TokenId, T::Hash, T::AccountId, T::AccountId),
        ///  Logged when a node is reclaimed.
        /// `[node,owner]`
        Reclaimed(T::Hash, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Not enough permissions to call functions.
        NoPermission,
        /// Not exist
        NotExist,
        /// Capacity is not enough to add new child nodes.
        CapacityNotEnough,
        /// If you want to `burn` your domain name, you first need to make
        /// sure there are no subdomains. If you just want to get your deposit
        /// back, you can consider using `reclaim`.
        ///
        /// Note: Using `reclaim` means that you are trading your domain name
        ///  to the official, who will refund your deposit.
        SubnodeNotClear,
        /// You may be burning a root node or an unknown node?
        BanBurnBaseNode,
        /// ERC721: approval to current owner
        ApprovalFailure,
        /// Pns official account is not initialized, please feedback to the official.
        OfficialNotInitiated,
    }

    // helper
    impl<T: Config> Pallet<T> {
        /// 验证，给定调用者和域名哈希，内部会验证是否有权限
        #[inline]
        pub fn verify(caller: &T::AccountId, node: T::Hash) -> DispatchResult {
            let owner = &nft::Pallet::<T>::tokens(T::ClassId::zero(), node)
                .ok_or(Error::<T>::NotExist)?
                .owner;

            Self::verify_with_owner(caller, node, owner)?;

            Ok(())
        }
        /// 验证，但给定owner
        #[inline]
        pub fn verify_with_owner(
            caller: &T::AccountId,
            node: T::Hash,
            owner: &T::AccountId,
        ) -> DispatchResult {
            // 确保调用者是owner或者有控制权限
            ensure!(
                caller == owner
                    || OperatorApprovals::<T>::contains_key(owner, caller)
                    || TokenApprovals::<T>::contains_key(node, caller),
                Error::<T>::NoPermission
            );

            Ok(())
        }
    }
    impl<T: Config> Pallet<T> {
        /// 销毁某个域名，调用者必须拥有该域名的控制权限
        #[cfg_attr(
            not(feature = "runtime-benchmarks"),
            frame_support::require_transactional
        )]
        pub(crate) fn burn(caller: T::AccountId, token: T::TokenId) -> DispatchResult {
            // 我们当前的域名只有一个类，因此使用的类ID就是0
            let class_id = T::ClassId::zero();
            // 尝试获取token，若token不存在则返回不存在的错误
            if let Some(token_info) = nft::Pallet::<T>::tokens(class_id, token) {
                // token当前的owner
                let token_owner = token_info.owner;
                // 确保当前token的子域名为零，否则不能销毁，该保证是为了更好的管理域名之间的从属关系
                // TODO: 另一个需求是用户在拥有大量子域名的同时，想要调用类似burn的功能
                // 应该准备另一个方法，可以将该域名交易给官方，而不是销毁它
                ensure!(token_info.data.children == 0, Error::<T>::SubnodeNotClear);
                // 验证权限
                Self::verify_with_owner(&caller, token, &token_owner)?;
                // 获取该域名的类型，当无法获取时，说明该域名既存在于nft内，但未存在于origin里
                // 因此只能是basenode，basenode是不允许销毁的
                if let Some(origin) = Origin::<T>::get(token) {
                    // 这里有两种可能，一种是该域名是一个根域名，另一种是该域名是一个子域名
                    // 子域名：要让子域名的origin的子域名数量减去一
                    // 根域名：需要清理掉该域名的注册信息，比如剩余时间，剩余的押金等
                    // 押金将会返还给域名的所有者
                    match origin {
                        DomainTracing::Origin(origin) => Self::sub_children(origin, class_id)?,
                        DomainTracing::Root => {
                            T::Registrar::clear_registrar_info(token, &token_owner)?;
                        }
                    }
                } else {
                    return Err(Error::<T>::BanBurnBaseNode.into());
                }
                // 调用nft模块的burn接口，销毁掉其nft数据
                nft::Pallet::<T>::burn(&token_owner, (class_id, token))?;
                // 保存销毁域名的事件
                Self::deposit_event(Event::<T>::TokenBurned(
                    class_id,
                    token,
                    token,
                    token_owner,
                    caller,
                ));
                Ok(())
            } else {
                Err(Error::<T>::NotExist.into())
            }
        }
        /// 铸造一个子域名
        #[cfg_attr(
            not(feature = "runtime-benchmarks"),
            frame_support::require_transactional
        )]
        pub(crate) fn mint_subname(
            // 当前域名的所有者，子域名是由一个已经存在的域名来铸造的
            owner: &T::AccountId,
            // 元数据，目前传递的值都是空
            metadata: Vec<u8>,
            // 当前域名的哈希
            node: T::Hash,
            // 当前需要铸造的子域名的名字部分的哈希
            // 例如 cupnfish.dot
            // 这里就需要的是 cupnfish.dot 的哈希值
            // TODO: 从label_node改名为subnode更合理
            subnode: T::Hash,
            // 铸造的子域名的所有者
            to: T::AccountId,
            // 当前域名的最大容量
            // 这个属性是存放在域名分发中心的
            capacity: u32,
            // `[maybe_pre_owner]`
            // 执行一些交易，这里主要是执行两个交易
            // 一、 收取 `to` 的注册费、押金
            // 二、 如果在铸造给新的 `to` 时，之前存在过所有者，则退还之前的押金给之前的所有者
            do_payments: impl FnOnce(Option<&T::AccountId>) -> DispatchResult,
        ) -> DispatchResult {
            // 默认域名的class id
            let class_id = T::ClassId::zero();
            // dot: hash 0xce159cf34380757d1932a8e4a74e85e85957b0a7a52d9c566c0a3c8d6133d0f7
            // [206, 21, 156, 243, 67, 128, 117, 125, 25, 50, 168, 228, 167, 78, 133, 232, 89, 87,
            // 176, 167, 165, 45, 156, 86, 108, 10, 60, 141, 97, 51, 208, 247]
            // 获取当前域名的信息
            if let Some(node_info) = nft::Pallet::<T>::tokens(class_id, node) {
                // 当前域名的所有者
                let node_owner = node_info.owner;
                // 验证调用者是否有权限操纵当前域名
                Self::verify_with_owner(owner, node, &node_owner)?;
                // 获取当前的子域名信息
                if let Some(info) = nft::Tokens::<T>::get(class_id, subnode) {
                    // 存在的情况下，说明该哈希现在是被注册的
                    // 检查一下是否处于可注册状态，比如之前的注册时间已过期
                    // TODO: 这里可能会出现一个bug
                    // 如果这不是一个根域名
                    // 只是一个被注册过的子域名，那么检查可用性是会返回错误的
                    // 但实际上我们不需要返回错误，而是将其所有权转移即可
                    T::Registrar::check_expires_registrable(subnode)?;

                    let from = info.owner;
                    // 做一些交易
                    // 一、返回押金给前一个拥有着
                    // 二、支付押金和注册费
                    do_payments(Some(&from))?;

                    nft::Pallet::<T>::transfer(&from, &to, (class_id, subnode))?;
                } else {
                    // 支付押金和注册费（在铸造子域名的分发接口内不执行任何支付操作）
                    do_payments(None)?;
                    // 因为不存在该域名，所以调用nft模块的铸造接口
                    nft::Pallet::<T>::mint(&to, (class_id, subnode), metadata, Default::default())?;
                    // 检查当前域名是否有足够的容量创建其子域名
                    if let Some(origin) = Origin::<T>::get(node) {
                        match origin {
                            // 如果当前域名是基于普通的root根域名
                            DomainTracing::Origin(origin) => {
                                // 检查该根域名是否可用
                                T::Registrar::check_expires_useable(origin)?;
                                // 检查并增加容量占有值
                                Self::add_children_with_check(origin, class_id, capacity)?;
                                // NOTE: 给当前node增加子域名数（该数有歧义，只能记录直属子域名的个数，隔代子域名的个数不会被记录）
                                Self::add_children(node, class_id)?;
                                // 插入当前子域名关系列表
                                Origin::<T>::insert(subnode, DomainTracing::Origin(origin));
                            }
                            // 如果是当前域名是基于basenode
                            DomainTracing::Root => {
                                // 检查并增加当前域名的子域名数量
                                Self::add_children_with_check(node, class_id, capacity)?;
                                // 插入子域名关系列表
                                Origin::<T>::insert(subnode, DomainTracing::Origin(node));
                            }
                        }
                    } else {
                        // 如果都不是则说明node是basenode
                        // 增加basenode的子域名数量
                        Self::add_children(node, class_id)?;
                        // 插入子域名关系表
                        Origin::<T>::insert(subnode, DomainTracing::Root);
                    }
                }
                // 保存代币铸造事件
                Self::deposit_event(Event::<T>::TokenMinted(class_id, subnode, node, to));

                Ok(())
            } else {
                // 返回node不存在的错误
                Err(Error::<T>::NotExist.into())
            }
        }
        /// 增加node子域名数量
        pub(crate) fn add_children(node: T::Hash, class_id: T::ClassId) -> DispatchResult {
            // 更改代币数据
            nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
                // 存在时自增即可
                if let Some(info) = data {
                    let node_children = info.data.children;
                    info.data.children = node_children + 1;
                    Ok(())
                } else {
                    Err(Error::<T>::NotExist.into())
                }
            })
        }
        // 增加node子域名数量，但会比较当前已有的数量是否大于容量
        fn add_children_with_check(
            node: T::Hash,
            class_id: T::ClassId,
            capacity: u32,
        ) -> DispatchResult {
            nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
                if let Some(info) = data {
                    let node_children = info.data.children;
                    // 比上一个方法多了教研容量的部分
                    ensure!(node_children < capacity, Error::<T>::CapacityNotEnough);
                    info.data.children = node_children + 1;
                    Ok(())
                } else {
                    Err(Error::<T>::NotExist.into())
                }
            })
        }
        /// Ensure `from` is a caller.
        /// 交易一个域名
        #[cfg_attr(
            not(feature = "runtime-benchmarks"),
            frame_support::require_transactional
        )]
        pub fn do_transfer(
            from: &T::AccountId,
            to: &T::AccountId,
            token: T::TokenId,
        ) -> DispatchResult {
            let class_id = T::ClassId::zero();
            // 获取该域名的信息
            let token_info =
                nft::Pallet::<T>::tokens(class_id, token).ok_or(Error::<T>::NotExist)?;
            // 获取该域名的实际所有者
            let owner = token_info.owner;
            // 验证from是否有权限交易该域名
            Self::verify_with_owner(from, token, &owner)?;
            // 获取关系
            if let Some(origin) = Origin::<T>::get(token) {
                match origin {
                    // 属于一个用户自建的子域名
                    DomainTracing::Origin(origin) => {
                        // 判断该子域名的始祖域名是否可续费
                        // 只要在可续费范围内均可交易出去
                        T::Registrar::check_expires_renewable(origin)?;
                    }
                    // 属于一个正常的根域名
                    DomainTracing::Root => {
                        // 检查当前域名是否处于可续费范围内
                        T::Registrar::check_expires_renewable(token)?;
                    }
                }
            } else {
                // 不存在则返回错误
                return Err(Error::<T>::NotExist.into());
            }
            // 调用nft模块的交易接口
            nft::Pallet::<T>::transfer(&owner, to, (class_id, token))?;
            // 保存交易事件
            Self::deposit_event(Event::<T>::Transferred(owner, to.clone(), class_id, token));

            Ok(())
        }
        /// 减去子域名数量
        fn sub_children(node: T::Hash, class_id: T::ClassId) -> DispatchResult {
            nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
                if let Some(info) = data {
                    let node_children = info.data.children;
                    info.data.children = node_children
                        .checked_sub(1)
                        .ok_or(sp_runtime::ArithmeticError::Overflow)?;
                    Ok(())
                } else {
                    Err(Error::<T>::NotExist.into())
                }
            })
        }
    }
    // 可直接使用不需要考虑权限问题的方法
    impl<T: Config> Pallet<T> {
        // 将caller的权限设置给operator
        #[inline(always)]
        fn do_set_approval_for_all(caller: T::AccountId, operator: T::AccountId, approved: bool) {
            OperatorApprovals::<T>::mutate_exists(&caller, &operator, |flag| {
                if approved {
                    flag.replace(())
                } else {
                    flag.take()
                }
            });
            Self::deposit_event(Event::ApprovalForAll(caller, operator, approved));
        }
        // 给Rpc调用
        // 获取一个caller的所有的operatore
        #[inline(always)]
        pub fn get_operators(caller: T::AccountId) -> Vec<T::AccountId> {
            OperatorApprovals::<T>::iter_prefix(caller)
                .map(|(operator, _)| operator)
                .collect()
        }
    }
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sharing your account permissions with others is a discreet operation,
        /// and when methods such as `reclaim` are called, the deposit is returned to the caller.
        /// 将调用者的操作权限分享给操作者
        #[pallet::weight(T::WeightInfo::approval_for_all(*approved))]
        pub fn approval_for_all(
            origin: OriginFor<T>,
            operator: <T::Lookup as StaticLookup>::Source,
            approved: bool,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let operator = T::Lookup::lookup(operator)?;

            Self::do_set_approval_for_all(caller, operator, approved);
            Ok(())
        }
        /// Set the resolver address.
        /// 设置resolver的ID
        #[pallet::weight(T::WeightInfo::set_resolver())]
        pub fn set_resolver(
            origin: OriginFor<T>,
            node: T::Hash,
            resolver: T::ResolverId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::verify(&caller, node)?;
            Resolver::<T>::mutate(node, |rs| *rs = resolver);
            Ok(())
        }
        /// Burn your node.
        ///
        /// Note: Using this does not refund your deposit,
        /// your deposit will be refunded to you
        /// when the domain is registered by another user.
        ///
        /// Ensure: The number of subdomains for this domain must be zero.
        /// 销毁一个域名
        /// TODO: bug，当该域名存在子域名时，不能正常销毁
        #[pallet::weight(T::WeightInfo::destroy())]
        pub fn destroy(origin: OriginFor<T>, node: T::Hash) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            Self::burn(caller, node)?;

            Ok(())
        }
        /// 设置官方账号
        #[pallet::weight(T::WeightInfo::set_official())]
        #[frame_support::transactional]
        pub fn set_official(origin: OriginFor<T>, official: T::AccountId) -> DispatchResult {
            // 必须是管理员才能调用
            let _who = T::ManagerOrigin::ensure_origin(origin)?;
            // 获取之前的官方账号
            let old_official = Official::<T>::take();
            // 设置为新的官方账号
            Official::<T>::put(&official);
            // 如果之前有老的官方账号，则把basenode的所有权交易到新的官方账号
            if let Some(old_official) = old_official {
                nft::Pallet::<T>::transfer(
                    &old_official,
                    &official,
                    (T::ClassId::zero(), T::Registrar::basenode()),
                )?;
            }
            // 该类的所有权也更改为新的（实际上这里并不重要，因为销毁类必须在发行量为零时才能销毁
            // 因此这里的更改只是让这部分数据看起来更合理
            nft::Classes::<T>::mutate(T::ClassId::zero(), |info| {
                if let Some(info) = info {
                    info.owner = official;
                }
            });

            Ok(())
        }
        /// 将一个域名授权给一个账户
        #[pallet::weight(T::WeightInfo::approve(*approved))]
        pub fn approve(
            origin: OriginFor<T>,
            to: T::AccountId,
            node: T::Hash,
            approved: bool,
        ) -> DispatchResult {
            // 调用者必须签名
            let sender = ensure_signed(origin)?;
            // 获取当前域名的所有者
            let owner = nft::Pallet::<T>::tokens(T::ClassId::zero(), node)
                .ok_or(Error::<T>::NotExist)?
                .owner;
            // 确保to不是所有者
            ensure!(to != owner, Error::<T>::ApprovalFailure);
            // 验证调用者权限
            Self::verify_with_owner(&sender, node, &owner)?;
            // 设置权限
            if approved {
                TokenApprovals::<T>::insert(node, to, ());
            } else {
                TokenApprovals::<T>::remove(node, to);
            }

            Ok(())
        }
    }
}

use frame_support::{
    dispatch::{DispatchResult, Weight},
    ensure,
};

pub trait WeightInfo {
    fn approval_for_all(approved: bool) -> Weight {
        if approved {
            Self::approval_for_all_true()
        } else {
            Self::approval_for_all_false()
        }
    }
    fn approval_for_all_true() -> Weight;
    fn approval_for_all_false() -> Weight;
    fn set_resolver() -> Weight;
    fn destroy() -> Weight;
    fn set_official() -> Weight;
    fn approve(approved: bool) -> Weight {
        if approved {
            Self::approve_true()
        } else {
            Self::approve_false()
        }
    }
    fn approve_true() -> Weight;
    fn approve_false() -> Weight;
}
// TODO: replace litentry
impl<T: pallet::Config> crate::traits::NFT<T::AccountId> for pallet::Pallet<T> {
    type ClassId = T::ClassId;

    type TokenId = T::TokenId;

    type Balance = u128;

    fn balance(who: &T::AccountId) -> Self::Balance {
        crate::nft::TokensByOwner::<T>::iter_prefix((who,)).count() as u128
    }

    fn owner(token: (Self::ClassId, Self::TokenId)) -> Option<T::AccountId> {
        crate::nft::Pallet::<T>::tokens(token.0, token.1).map(|t| t.owner)
    }
    #[cfg_attr(
        not(feature = "runtime-benchmarks"),
        frame_support::require_transactional
    )]
    fn transfer(
        from: &T::AccountId,
        to: &T::AccountId,
        token: (Self::ClassId, Self::TokenId),
    ) -> DispatchResult {
        use sp_runtime::traits::Zero;
        ensure!(token.0 == T::ClassId::zero(), Error::<T>::NotExist);

        Self::do_transfer(from, to, token.1)
    }
}

impl<T: pallet::Config> crate::traits::Registry for pallet::Pallet<T> {
    type AccountId = T::AccountId;
    type Hash = T::Hash;

    #[cfg_attr(
        not(feature = "runtime-benchmarks"),
        frame_support::require_transactional
    )]
    fn mint_subname(
        node_owner: &Self::AccountId,
        node: Self::Hash,
        subnode: Self::Hash,
        to: Self::AccountId,
        capacity: u32,
        do_payments: impl FnOnce(Option<&T::AccountId>) -> DispatchResult,
    ) -> DispatchResult {
        let metadata = Vec::with_capacity(0);
        Self::mint_subname(
            node_owner,
            metadata,
            node,
            subnode,
            to,
            capacity,
            do_payments,
        )
    }

    /// 方便后续判断权限
    fn available(caller: &Self::AccountId, node: Self::Hash) -> DispatchResult {
        pallet::Pallet::<T>::verify(caller, node)
    }

    #[frame_support::require_transactional]
    fn transfer(from: &Self::AccountId, to: &Self::AccountId, node: Self::Hash) -> DispatchResult {
        Self::do_transfer(from, to, node)
    }
}

impl<T: Config> crate::traits::Official for pallet::Pallet<T> {
    type AccountId = T::AccountId;

    fn get_official_account() -> Result<Self::AccountId, DispatchError> {
        Official::<T>::get().ok_or_else(|| Error::<T>::OfficialNotInitiated.into())
    }
}

impl WeightInfo for () {
    fn approval_for_all_true() -> Weight {
        0
    }

    fn approval_for_all_false() -> Weight {
        0
    }

    fn set_resolver() -> Weight {
        0
    }

    fn destroy() -> Weight {
        0
    }

    fn set_official() -> Weight {
        0
    }

    fn approve_true() -> Weight {
        0
    }

    fn approve_false() -> Weight {
        0
    }
}
