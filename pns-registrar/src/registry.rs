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
//! - `burn` - destroy a domain, return it to the owner if there is a deposit, requires the domain's operational privileges
//! - `set_official` - Set official account, needs manager privileges
//! - `approve` - share the permission of a domain to another account, requires the permission of the domain

pub use pallet::*;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::{nft, traits::Registrar};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::EnsureOrigin;
    use frame_system::{ensure_signed, pallet_prelude::*};
    use pns_types::{DomainHash, DomainTracing, Record};
    use sp_runtime::traits::{StaticLookup, Zero};

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + crate::nft::Config<ClassData = (), TokenData = Record, TokenId = DomainHash>
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WeightInfo: WeightInfo;

        type Registrar: Registrar<AccountId = Self::AccountId>;

        type ResolverId: Parameter + Default + MaxEncodedLen;

        type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// `name_hash` -> (`origin`,`parent`) or `origin`
    #[pallet::storage]
    pub type RuntimeOrigin<T: Config> = StorageMap<_, Twox64Concat, DomainHash, DomainTracing>;
    /// `name_hash` -> `resolver_id`
    #[pallet::storage]
    pub type Resolver<T: Config> =
        StorageMap<_, Twox64Concat, DomainHash, T::ResolverId, ValueQuery>;
    /// `official`
    #[pallet::storage]
    pub type Official<T: Config> = StorageValue<_, T::AccountId>;

    /// (`owner`,`account`) if `account` is `operater` -> ()
    #[pallet::storage]
    pub type OperatorApprovals<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, T::AccountId, (), ValueQuery>;

    /// (`node`,`account`) `node` -> `account`
    #[pallet::storage]
    pub type TokenApprovals<T: Config> =
        StorageDoubleMap<_, Twox64Concat, DomainHash, Twox64Concat, T::AccountId, (), ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub origin: Vec<(DomainHash, DomainTracing)>,
        pub official: Option<T::AccountId>,
        pub operators: Vec<(T::AccountId, T::AccountId)>,
        pub token_approvals: Vec<(DomainHash, T::AccountId)>,
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
                RuntimeOrigin::<T>::insert(node, origin);
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
        /// Logged when the resolver for a node changes.
        NewResolver {
            node: DomainHash,
            resolver: T::ResolverId,
        },
        /// Logged when an operator is added or removed.
        ApprovalForAll {
            owner: T::AccountId,
            operator: T::AccountId,
            approved: bool,
        },
        /// Logged when a node is traded.
        Transferred {
            from: T::AccountId,
            to: T::AccountId,
            class_id: T::ClassId,
            token_id: T::TokenId,
        },
        /// Logged when a node is minted.
        TokenMinted {
            class_id: T::ClassId,
            token_id: T::TokenId,
            node: DomainHash,
            owner: T::AccountId,
        },
        /// Logged when a node is burned.
        TokenBurned {
            class_id: T::ClassId,
            token_id: T::TokenId,
            node: DomainHash,
            owner: T::AccountId,
            caller: T::AccountId,
        },
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
        #[inline]
        pub fn verify(caller: &T::AccountId, node: DomainHash) -> DispatchResult {
            let owner = &nft::Pallet::<T>::tokens(T::ClassId::zero(), node)
                .ok_or(Error::<T>::NotExist)?
                .owner;

            Self::verify_with_owner(caller, node, owner)?;

            Ok(())
        }

        #[inline]
        pub fn verify_with_owner(
            caller: &T::AccountId,
            node: DomainHash,
            owner: &T::AccountId,
        ) -> DispatchResult {
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
        pub(crate) fn do_burn(caller: T::AccountId, token: T::TokenId) -> DispatchResult {
            let class_id = T::ClassId::zero();
            let Some(token_info) = nft::Pallet::<T>::tokens(class_id, token) else {
                return Err(Error::<T>::NotExist.into())
            };

            let token_owner = token_info.owner;
            ensure!(token_info.data.children == 0, Error::<T>::SubnodeNotClear);

            Self::verify_with_owner(&caller, token, &token_owner)?;

            let Some(origin) = RuntimeOrigin::<T>::get(token) else {
                    return Err(Error::<T>::BanBurnBaseNode.into())
            };

            match origin {
                DomainTracing::RuntimeOrigin(origin) => Self::sub_children(origin, class_id)?,
                DomainTracing::Root => {
                    T::Registrar::clear_registrar_info(token, &token_owner)?;
                }
            }

            nft::Pallet::<T>::burn(&token_owner, (class_id, token))?;

            Self::deposit_event(Event::<T>::TokenBurned {
                class_id,
                token_id: token,
                node: token,
                owner: token_owner,
                caller,
            });
            Ok(())
        }

        #[cfg_attr(
            not(feature = "runtime-benchmarks"),
            frame_support::require_transactional
        )]
        pub(crate) fn mint_subname(
            owner: &T::AccountId,
            metadata: Vec<u8>,
            node: DomainHash,
            label_node: DomainHash,
            to: T::AccountId,
            capacity: u32,
            // `[maybe_pre_owner]`
            do_payments: impl FnOnce(Option<&T::AccountId>) -> DispatchResult,
        ) -> DispatchResult {
            let class_id = T::ClassId::zero();
            // dot: hash 0xce159cf34380757d1932a8e4a74e85e85957b0a7a52d9c566c0a3c8d6133d0f7
            // [206, 21, 156, 243, 67, 128, 117, 125, 25, 50, 168, 228, 167, 78, 133, 232, 89, 87,
            // 176, 167, 165, 45, 156, 86, 108, 10, 60, 141, 97, 51, 208, 247]
            let Some(node_info) = nft::Pallet::<T>::tokens(class_id, node) else {
                return Err(Error::<T>::NotExist.into());
            };

            let node_owner = node_info.owner;

            Self::verify_with_owner(owner, node, &node_owner)?;

            if let Some(info) = nft::Tokens::<T>::get(class_id, label_node) {
                T::Registrar::check_expires_registrable(label_node)?;

                let from = info.owner;

                do_payments(Some(&from))?;

                nft::Pallet::<T>::transfer(&from, &to, (class_id, label_node))?;
            } else {
                do_payments(None)?;

                nft::Pallet::<T>::mint(&to, (class_id, label_node), metadata, Default::default())?;

                if let Some(origin) = RuntimeOrigin::<T>::get(node) {
                    match origin {
                        DomainTracing::RuntimeOrigin(origin) => {
                            T::Registrar::check_expires_useable(origin)?;

                            Self::add_children_with_check(origin, class_id, capacity)?;

                            Self::add_children(node, class_id)?;

                            RuntimeOrigin::<T>::insert(
                                label_node,
                                DomainTracing::RuntimeOrigin(origin),
                            );
                        }
                        DomainTracing::Root => {
                            Self::add_children_with_check(node, class_id, capacity)?;

                            RuntimeOrigin::<T>::insert(
                                label_node,
                                DomainTracing::RuntimeOrigin(node),
                            );
                        }
                    }
                } else {
                    Self::add_children(node, class_id)?;

                    RuntimeOrigin::<T>::insert(label_node, DomainTracing::Root);
                }
            }
            Self::deposit_event(Event::<T>::TokenMinted {
                class_id,
                token_id: label_node,
                node,
                owner: to,
            });

            Ok(())
        }
        pub(crate) fn add_children(node: DomainHash, class_id: T::ClassId) -> DispatchResult {
            nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
                let Some(info) = data else {
                    return Err(Error::<T>::NotExist.into())
                };

                let node_children = info.data.children;
                info.data.children = node_children + 1;
                Ok(())
            })
        }
        fn add_children_with_check(
            node: DomainHash,
            class_id: T::ClassId,
            capacity: u32,
        ) -> DispatchResult {
            nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
                let Some(info) = data else {
                    return Err(Error::<T>::NotExist.into())
                };
                let node_children = info.data.children;
                ensure!(node_children < capacity, Error::<T>::CapacityNotEnough);
                info.data.children = node_children + 1;
                Ok(())
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

            Self::verify_with_owner(from, token, &owner)?;

            let Some(origin) = RuntimeOrigin::<T>::get(token) else {
                return Err(Error::<T>::NotExist.into())
            };

            match origin {
                DomainTracing::RuntimeOrigin(origin) => {
                    T::Registrar::check_expires_renewable(origin)?;
                }
                DomainTracing::Root => {
                    T::Registrar::check_expires_renewable(token)?;
                }
            }

            nft::Pallet::<T>::transfer(&owner, to, (class_id, token))?;

            Self::deposit_event(Event::<T>::Transferred {
                from: owner,
                to: to.clone(),
                class_id,
                token_id: token,
            });

            Ok(())
        }

        fn sub_children(node: DomainHash, class_id: T::ClassId) -> DispatchResult {
            nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
                let Some(info) = data else {
                    return  Err(Error::<T>::NotExist.into())
                };

                let node_children = info.data.children;
                info.data.children = node_children
                    .checked_sub(1)
                    .ok_or(sp_runtime::ArithmeticError::Overflow)?;
                Ok(())
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
            Self::deposit_event(Event::ApprovalForAll {
                owner: caller,
                operator,
                approved,
            });
        }
        // 给Rpc调用
        // #[inline(always)]
        // pub fn get_operators(caller: T::AccountId) -> Vec<T::AccountId> {
        //     OperatorApprovals::<T>::iter_prefix(caller)
        //         .map(|(operator, _)| operator)
        //         .collect()
        // }
    }
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sharing your account permissions with others is a discreet operation,
        /// and when methods such as `reclaim` are called, the deposit is returned to the caller.
        #[pallet::call_index(0)]
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
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_resolver())]
        pub fn set_resolver(
            origin: OriginFor<T>,
            node: DomainHash,
            resolver: T::ResolverId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::verify(&caller, node)?;
            Resolver::<T>::mutate(node, |rs| *rs = resolver.clone());

            Self::deposit_event(Event::<T>::NewResolver { node, resolver });
            Ok(())
        }
        /// Burn your node.
        ///
        /// Note: Using this does not refund your deposit,
        /// your deposit will be refunded to you
        /// when the domain is registered by another user.
        ///
        /// Ensure: The number of subdomains for this domain must be zero.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::burn())]
        pub fn burn(origin: OriginFor<T>, node: DomainHash) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            Self::do_burn(caller, node)?;

            Ok(())
        }
        #[pallet::call_index(3)]
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
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::approve(*approved))]
        pub fn approve(
            origin: OriginFor<T>,
            to: T::AccountId,
            node: DomainHash,
            approved: bool,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            let owner = nft::Pallet::<T>::tokens(T::ClassId::zero(), node)
                .ok_or(Error::<T>::NotExist)?
                .owner;

            ensure!(to != owner, Error::<T>::ApprovalFailure);

            Self::verify_with_owner(&sender, node, &owner)?;

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
use pns_types::DomainHash;
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
    fn burn() -> Weight;
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

    #[cfg_attr(
        not(feature = "runtime-benchmarks"),
        frame_support::require_transactional
    )]
    fn mint_subname(
        node_owner: &Self::AccountId,
        node: DomainHash,
        label_node: DomainHash,
        to: Self::AccountId,
        capacity: u32,
        do_payments: impl FnOnce(Option<&T::AccountId>) -> DispatchResult,
    ) -> DispatchResult {
        let metadata = Vec::with_capacity(0);
        Self::mint_subname(
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
    fn available(caller: &Self::AccountId, node: DomainHash) -> DispatchResult {
        pallet::Pallet::<T>::verify(caller, node)
    }

    #[frame_support::require_transactional]
    fn transfer(from: &Self::AccountId, to: &Self::AccountId, node: DomainHash) -> DispatchResult {
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
        Weight::zero()
    }

    fn approval_for_all_false() -> Weight {
        Weight::zero()
    }

    fn set_resolver() -> Weight {
        Weight::zero()
    }

    fn burn() -> Weight {
        Weight::zero()
    }

    fn set_official() -> Weight {
        Weight::zero()
    }

    fn approve_true() -> Weight {
        Weight::zero()
    }

    fn approve_false() -> Weight {
        Weight::zero()
    }
}
