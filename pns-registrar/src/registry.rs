use alloc::vec::Vec;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::{nft, traits::Registrar};
	use codec::FullCodec;
	use frame_support::{pallet_prelude::*, traits::Get, Blake2_128Concat};
	use frame_system::{ensure_signed, pallet_prelude::*};
	use scale_info::TypeInfo;
	use serde::{Deserialize, Serialize};
	use sp_runtime::traits::Zero;

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

		#[pallet::constant]
		type Official: Get<Self::AccountId>;

		#[pallet::constant]
		type DefaultMetadata: Get<Vec<u8>>;

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
			+ TypeInfo;
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

	// node -> (origin,parent)
	#[pallet::storage]
	pub type Origin<T: Config> = StorageMap<_, Twox64Concat, T::Hash, DomainTracing<T::Hash>>;

	#[pallet::storage]
	pub type Resolver<T: Config> = StorageMap<_, Twox64Concat, T::Hash, T::ResolverId, ValueQuery>;

	#[derive(Encode, Decode, PartialEq, Eq, RuntimeDebug, Clone, TypeInfo)]
	pub enum DomainTracing<Hash> {
		OriginAndParent(Hash, Hash),
		Origin(Hash),
		Root,
	}

	#[pallet::storage]
	pub type Operators<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::AccountId,
		(),
		ValueQuery,
	>;

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
		/// `[from,to,class_id,token_id]`
		Transferred(T::AccountId, T::AccountId, T::ClassId, T::TokenId),
		/// `[class_id,token_id,node,owner]`
		TokenMinted(T::ClassId, T::TokenId, T::Hash, T::AccountId),
		/// `[class_id,token_id,node,owner,caller]`
		TokenBurned(T::ClassId, T::TokenId, T::Hash, T::AccountId, T::AccountId),
		/// `[node,owner]`
		Reclaimed(T::Hash, T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Not enough permissions to call functions.
		NoPermission,
		/// Not exist
		NotExist,
		/// Node is already existed.
		NodeExisted,
		CapacityNotEnough,
		SubnodeNotClear,
		Filtered,
		BurnFailed,
		BanBurnBaseNode,
	}

	// helper
	impl<T: Config> Pallet<T> {
		/// 检查权限，只有域名的所有者,或者域名所有者的操作者才能继续后续操作
		/// 其中域名的初始所有者职能操作未分配的域名
		/// 已分配的域名在过期后会被回收
		/// 即被分配的域名只能在被回收之后才能被初始所有者操作
		pub(crate) fn authorised(caller: &T::AccountId, node: T::Hash) -> DispatchResult {
			let owner = &nft::Pallet::<T>::tokens(T::ClassId::zero(), node)
				.ok_or_else(|| Error::<T>::NotExist)?
				.owner;

			ensure!(
				caller == owner || Operators::<T>::contains_key(owner, caller),
				Error::<T>::NoPermission
			);

			Ok(())
		}
	}
	// 需要验证权限之后才能调用的方法
	impl<T: Config> Pallet<T> {
		// 调用前需要清除注册中心的数据
		pub(crate) fn _burn(caller: T::AccountId, token: T::TokenId) -> DispatchResult {
			let class_id = T::ClassId::zero();
			if let Some(token_info) = nft::Pallet::<T>::tokens(class_id, token) {
				let token_owner = token_info.owner;
				ensure!(token_info.data.children == 0, Error::<T>::SubnodeNotClear);
				ensure!(
					token_owner == caller || Operators::<T>::contains_key(&token_owner, &caller),
					Error::<T>::NoPermission
				);

				if let Some(origin) = Origin::<T>::get(token) {
					match origin {
						DomainTracing::OriginAndParent(origin, parent) => {
							Self::sub_children(origin, class_id)?;
							Self::sub_children(parent, class_id)?;
						},
						DomainTracing::Origin(origin) => Self::sub_children(origin, class_id)?,
						DomainTracing::Root => {
							T::Registrar::clear_registrar_info(token, &token_owner)?;
						},
					}
				} else {
					return Err(Error::<T>::BanBurnBaseNode.into())
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
		// 在创世区块里需要做初始化
		// 0是拍卖类域名
		// 1是兑换码类域名
		// 2是正常注册类域名
		// 3是子域名，子域名比较特殊，到期时间与其根域名挂钩
		// 不过目前全部按照0来处理即可
		fn _create_collections(metadata: Vec<u8>) -> Result<T::ClassId, DispatchError> {
			nft::Pallet::<T>::create_class(&T::Official::get(), metadata, ())
		}
		#[frame_support::require_transactional]
		pub(crate) fn _mint_subname(
			owner: &T::AccountId,
			metadata: Vec<u8>,
			node: T::Hash,
			label_node: T::Hash,
			to: T::AccountId,
			capacity: u32,
			do_payments: impl FnOnce(Option<&T::AccountId>) -> DispatchResult,
		) -> DispatchResult {
			let class_id = T::ClassId::zero();
			// dot: hash 0xce159cf34380757d1932a8e4a74e85e85957b0a7a52d9c566c0a3c8d6133d0f7
			// [206, 21, 156, 243, 67, 128, 117, 125, 25, 50, 168, 228, 167, 78, 133, 232, 89, 87,
			// 176, 167, 165, 45, 156, 86, 108, 10, 60, 141, 97, 51, 208, 247]
			if let Some(node_info) = nft::Pallet::<T>::tokens(class_id, node) {
				let node_owner = node_info.owner;
				ensure!(
					owner == &node_owner || Operators::<T>::contains_key(node_owner, owner),
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
							DomainTracing::OriginAndParent(origin, _) |
							DomainTracing::Origin(origin) => {
								T::Registrar::check_expires_useable(origin)?;

								Self::add_children_with_check(origin, class_id, capacity)?;

								Self::add_children(node, class_id)?;

								Origin::<T>::insert(
									label_node,
									DomainTracing::OriginAndParent(origin, node),
								);
							},
							DomainTracing::Root => {
								Self::add_children_with_check(node, class_id, capacity)?;

								Origin::<T>::insert(label_node, DomainTracing::Origin(node));
							},
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
		fn add_children(node: T::Hash, class_id: T::ClassId) -> DispatchResult {
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

		// 外部调用时确保from就是caller
		#[frame_support::require_transactional]
		pub fn do_transfer(
			from: &T::AccountId,
			to: &T::AccountId,
			token: T::TokenId,
		) -> DispatchResult {
			let class_id = T::ClassId::zero();
			let token_info =
				nft::Pallet::<T>::tokens(class_id, token).ok_or_else(|| Error::<T>::NotExist)?;

			let owner = token_info.owner;

			ensure!(
				&owner == from || Operators::<T>::contains_key(&owner, &from),
				Error::<T>::NoPermission
			);

			if let Some(origin) = Origin::<T>::get(token) {
				match origin {
					DomainTracing::OriginAndParent(origin, _) | DomainTracing::Origin(origin) => {
						T::Registrar::check_expires_renewable(origin)?;
					},
					DomainTracing::Root => {
						T::Registrar::check_expires_renewable(token)?;
					},
				}
			} else {
				return Err(Error::<T>::NotExist.into())
			}

			nft::Pallet::<T>::transfer(&owner, to, (class_id, token))?;

			Self::deposit_event(Event::<T>::Transferred(owner, to.clone(), class_id, token));

			Ok(())
		}

		fn sub_children(node: T::Hash, class_id: T::ClassId) -> DispatchResult {
			nft::Tokens::<T>::mutate(class_id, node, |data| -> DispatchResult {
				if let Some(info) = data {
					let node_children = info.data.children;
					info.data.children = node_children - 1;
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
			Operators::<T>::mutate_exists(&caller, &operator, |flag| {
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
			Operators::<T>::iter_prefix(caller).map(|(operator, _)| operator).collect()
		}

		// 给Rpc调用
		#[inline(always)]
		pub fn subnode(node: T::Hash, label: T::Hash) -> T::Hash {
			let context = sp_io::hashing::keccak_256(&codec::Encode::encode(&(node, label)));

			sp_core::convert_hash::<T::Hash, [u8; 32]>(&context)
		}
	}
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::set_approval_for_all())]
		pub fn set_approval_for_all(
			origin: OriginFor<T>,
			operator: T::AccountId,
			approved: bool,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			Self::do_set_approval_for_all(caller, operator, approved);
			Ok(())
		}
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

		#[pallet::weight(T::WeightInfo::destroy())]
		pub fn destroy(origin: OriginFor<T>, node: T::Hash) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			Self::_burn(caller, node)?;

			Ok(())
		}
	}
}

use frame_support::{
	dispatch::{DispatchResult, Weight},
	ensure,
};

pub trait WeightInfo {
	fn set_approval_for_all() -> Weight;
	fn set_resolver() -> Weight;
	fn destroy() -> Weight;
}

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
	#[frame_support::require_transactional]
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

	fn get_official_account() -> Self::AccountId {
		use frame_support::traits::Get;
		T::Official::get()
	}

	#[frame_support::require_transactional]
	fn mint_subname(
		node_owner: &Self::AccountId,
		node: Self::Hash,
		label_node: Self::Hash,
		to: Self::AccountId,
		capacity: u32,
		do_payments: impl FnOnce(Option<&T::AccountId>) -> DispatchResult,
	) -> DispatchResult {
		use frame_support::traits::Get;
		let metadata = <T as pallet::Config>::DefaultMetadata::get();
		Self::_mint_subname(node_owner, metadata, node, label_node, to, capacity, do_payments)
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
