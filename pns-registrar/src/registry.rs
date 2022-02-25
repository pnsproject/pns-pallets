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
    /// [`owner`,`account`] if `account` is `operater` -> ()
    #[pallet::storage]
    pub type OperatorApprovals<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, T::AccountId, (), ValueQuery>;

    /// [`node`,`account`] `node` -> `account`
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
        pub fn authorised(caller: &T::AccountId, node: T::Hash) -> DispatchResult {
            let owner = &nft::Pallet::<T>::tokens(T::ClassId::zero(), node)
                .ok_or(Error::<T>::NotExist)?
                .owner;

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
        pub(crate) fn _burn(caller: T::AccountId, token: T::TokenId) -> DispatchResult {
            let class_id = T::ClassId::zero();
            if let Some(token_info) = nft::Pallet::<T>::tokens(class_id, token) {
                let token_owner = token_info.owner;
                ensure!(token_info.data.children == 0, Error::<T>::SubnodeNotClear);
                ensure!(
                    token_owner == caller
                        || OperatorApprovals::<T>::contains_key(&token_owner, &caller)
                        || TokenApprovals::<T>::contains_key(token, &caller),
                    Error::<T>::NoPermission
                );

                if let Some(origin) = Origin::<T>::get(token) {
                    match origin {
                        DomainTracing::Origin(origin) => Self::sub_children(origin, class_id)?,
                        DomainTracing::Root => {
                            T::Registrar::clear_registrar_info(token, &token_owner)?;
                        }
                    }
                } else {
                    return Err(Error::<T>::BanBurnBaseNode.into());
                }

                nft::Pallet::<T>::burn(&token_owner, (class_id, token))?;

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

        #[cfg_attr(
            not(feature = "runtime-benchmarks"),
            frame_support::require_transactional
        )]
        pub(crate) fn _mint_subname(
            owner: &T::AccountId,
            metadata: Vec<u8>,
            node: T::Hash,
            label_node: T::Hash,
            to: T::AccountId,
            capacity: u32,
            // `[pre_owner]`
            do_payments: impl FnOnce(Option<&T::AccountId>) -> DispatchResult,
        ) -> DispatchResult {
            let class_id = T::ClassId::zero();
            // dot: hash 0xce159cf34380757d1932a8e4a74e85e85957b0a7a52d9c566c0a3c8d6133d0f7
            // [206, 21, 156, 243, 67, 128, 117, 125, 25, 50, 168, 228, 167, 78, 133, 232, 89, 87,
            // 176, 167, 165, 45, 156, 86, 108, 10, 60, 141, 97, 51, 208, 247]
            if let Some(node_info) = nft::Pallet::<T>::tokens(class_id, node) {
                let node_owner = node_info.owner;
                ensure!(
                    owner == &node_owner
                        || OperatorApprovals::<T>::contains_key(node_owner, owner)
                        || TokenApprovals::<T>::contains_key(node, owner),
                    Error::<T>::NoPermission
                );

                if let Some(info) = nft::Tokens::<T>::get(class_id, label_node) {
                    T::Registrar::check_expires_registrable(label_node)?;

                    let from = info.owner;

                    do_payments(Some(&from))?;

                    nft::Pallet::<T>::transfer(&from, &to, (class_id, label_node))?;
                } else {
                    do_payments(None)?;

                    nft::Pallet::<T>::mint(
                        &to,
                        (class_id, label_node),
                        metadata,
                        Default::default(),
                    )?;

                    if let Some(origin) = Origin::<T>::get(node) {
                        match origin {
                            DomainTracing::Origin(origin) => {
                                T::Registrar::check_expires_useable(origin)?;

                                Self::add_children_with_check(origin, class_id, capacity)?;

                                Self::add_children(node, class_id)?;

                                Origin::<T>::insert(label_node, DomainTracing::Origin(origin));
                            }
                            DomainTracing::Root => {
                                Self::add_children_with_check(node, class_id, capacity)?;

                                Origin::<T>::insert(label_node, DomainTracing::Origin(node));
                            }
                        }
                    } else {
                        Self::add_children(node, class_id)?;

                        Origin::<T>::insert(label_node, DomainTracing::Root);
                    }
                }
                Self::deposit_event(Event::<T>::TokenMinted(class_id, label_node, node, to));

                Ok(())
            } else {
                Err(Error::<T>::NotExist.into())
            }
        }
        pub(crate) fn add_children(node: T::Hash, class_id: T::ClassId) -> DispatchResult {
            nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
                if let Some(info) = data {
                    let node_children = info.data.children;
                    info.data.children = node_children + 1;
                    Ok(())
                } else {
                    Err(Error::<T>::NotExist.into())
                }
            })
        }
        fn add_children_with_check(
            node: T::Hash,
            class_id: T::ClassId,
            capacity: u32,
        ) -> DispatchResult {
            nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
                if let Some(info) = data {
                    let node_children = info.data.children;
                    ensure!(node_children < capacity, Error::<T>::CapacityNotEnough);
                    info.data.children = node_children + 1;
                    Ok(())
                } else {
                    Err(Error::<T>::NotExist.into())
                }
            })
        }
        /// Ensure `from` is a caller.
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
            let token_info =
                nft::Pallet::<T>::tokens(class_id, token).ok_or(Error::<T>::NotExist)?;

            let owner = token_info.owner;

            ensure!(
                &owner == from
                    || OperatorApprovals::<T>::contains_key(&owner, &from)
                    || TokenApprovals::<T>::contains_key(token, &from),
                Error::<T>::NoPermission
            );

            if let Some(origin) = Origin::<T>::get(token) {
                match origin {
                    DomainTracing::Origin(origin) => {
                        T::Registrar::check_expires_renewable(origin)?;
                    }
                    DomainTracing::Root => {
                        T::Registrar::check_expires_renewable(token)?;
                    }
                }
            } else {
                return Err(Error::<T>::NotExist.into());
            }

            nft::Pallet::<T>::transfer(&owner, to, (class_id, token))?;

            Self::deposit_event(Event::<T>::Transferred(owner, to.clone(), class_id, token));

            Ok(())
        }

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
        #[pallet::weight(T::WeightInfo::set_resolver())]
        pub fn set_resolver(
            origin: OriginFor<T>,
            node: T::Hash,
            resolver: T::ResolverId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::authorised(&caller, node)?;
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
        #[pallet::weight(T::WeightInfo::destroy())]
        pub fn destroy(origin: OriginFor<T>, node: T::Hash) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            Self::_burn(caller, node)?;

            Ok(())
        }

        #[pallet::weight(T::WeightInfo::set_official())]
        #[frame_support::transactional]
        pub fn set_official(origin: OriginFor<T>, official: T::AccountId) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;
            let old_official = Official::<T>::take();

            Official::<T>::put(&official);

            if let Some(old_official) = old_official {
                nft::Pallet::<T>::transfer(
                    &old_official,
                    &official,
                    (T::ClassId::zero(), T::Registrar::basenode()),
                )?;
            }

            nft::Classes::<T>::mutate(T::ClassId::zero(), |info| {
                if let Some(info) = info {
                    info.owner = official;
                }
            });

            Ok(())
        }

        #[pallet::weight(T::WeightInfo::approve(*approved))]
        pub fn approve(
            origin: OriginFor<T>,
            to: T::AccountId,
            node: T::Hash,
            approved: bool,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            let owner = nft::Pallet::<T>::tokens(T::ClassId::zero(), node)
                .ok_or(Error::<T>::NotExist)?
                .owner;

            ensure!(to != owner, Error::<T>::ApprovalFailure);

            ensure!(
                sender == owner
                    || OperatorApprovals::<T>::contains_key(&owner, &sender)
                    || TokenApprovals::<T>::contains_key(node, &sender),
                Error::<T>::NoPermission
            );

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
        label_node: Self::Hash,
        to: Self::AccountId,
        capacity: u32,
        do_payments: impl FnOnce(Option<&T::AccountId>) -> DispatchResult,
    ) -> DispatchResult {
        let metadata = Vec::with_capacity(0);
        Self::_mint_subname(
            node_owner,
            metadata,
            node,
            label_node,
            to,
            capacity,
            do_payments,
        )
    }

    /// 方便后续判断权限
    fn available(caller: &Self::AccountId, node: Self::Hash) -> DispatchResult {
        pallet::Pallet::<T>::authorised(caller, node)
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
