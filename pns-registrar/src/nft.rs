//! # Non Fungible Token
//! The module provides implementations for non-fungible-token.
//!
//! - [`Config`](./trait.Config.html)
//! - [`Call`](./enum.Call.html)
//! - [`Module`](./struct.Module.html)
//!
//! ## Overview
//!
//! This module provides basic functions to create and manager
//! NFT(non fungible token) such as `create_class`, `transfer`, `mint`, `burn`.

//! ### Module Functions
//!
//! - `create_class` - Create NFT(non fungible token) class
//! - `transfer` - Transfer NFT(non fungible token) to another account.
//! - `mint` - Mint NFT(non fungible token)
//! - `burn` - Burn NFT(non fungible token)
//! - `destroy_class` - Destroy NFT(non fungible token) class

//! ### PNS Added
//!
//! The current `pns-pallets` have had the following magic changes made to them.
//!
//! 1. Removed the `token id` that relied on the counter, and
//! instead stored it via an externally provided `token id`.
//!
//! 2. added `total id` to replace the previous `token id` total.
//!  (One drawback is that the maximum is only `u128` because of
//! the `AtLeast32BitUnsigned` limit, however, `token id` is usually `H256`,
//!  i.e., the module will overflow due to too many `total`s when it runs
//! for a long time to a future date.)
//!
//! 3. Changed the function signature of `mint`,
//! which adds `token id` to the parameters of the signature,
//! and during `mint`, it no longer generates `token id` by counter,
//! but stores it with the incoming `token id`.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{ensure, pallet_prelude::*, traits::Get, BoundedVec, Parameter};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{
        AtLeast32BitUnsigned, CheckedAdd, CheckedSub, MaybeSerializeDeserialize, Member, One, Zero,
    },
    ArithmeticError, DispatchError, DispatchResult, RuntimeDebug,
};
use sp_std::vec::Vec;

/// Class info
/// 该类的信息
#[derive(Encode, Decode, Clone, Eq, PartialEq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct ClassInfo<TotalId, AccountId, Data, ClassMetadataOf> {
    /// Class metadata
    /// 该类的元数据
    pub metadata: ClassMetadataOf,
    /// Total issuance for the class
    /// 该类的发行总数
    pub total_issuance: TotalId,
    /// Class owner
    /// 该类的所有者
    pub owner: AccountId,
    /// Class Properties
    /// 该类的属性
    pub data: Data,
}

/// Token info
/// 该代币的信息
#[derive(Encode, Decode, Clone, Eq, PartialEq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct TokenInfo<AccountId, Data, TokenMetadataOf> {
    /// Token metadata
    /// 该代币的元数据
    pub metadata: TokenMetadataOf,
    /// Token owner
    /// 该代币的所有者
    pub owner: AccountId,
    /// Token Properties
    /// 该代币的属性
    pub data: Data,
}

pub use module::*;

#[frame_support::pallet]
pub mod module {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The class ID type
        /// 该类所用的ID类型
        type ClassId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy;
        /// The total ID type
        /// 发行数量所用的类型
        type TotalId: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize;
        /// The token ID type
        /// 该代币所用的ID类型
        type TokenId: Parameter + Member + Default + Copy + MaybeSerializeDeserialize;
        /// The class properties type
        /// 该类的属性类型
        type ClassData: Parameter + Member + MaybeSerializeDeserialize;
        /// The token properties type
        /// 该代币的属性类型
        type TokenData: Parameter + Member + MaybeSerializeDeserialize;
        /// The maximum size of a class's metadata
        /// 该类的元数据的最大容量
        type MaxClassMetadata: Get<u32>;
        /// The maximum size of a token's metadata
        /// 该代币的元数据的最大容量
        type MaxTokenMetadata: Get<u32>;
    }
    /// 该类元数据
    pub type ClassMetadataOf<T> = BoundedVec<u8, <T as Config>::MaxClassMetadata>;
    /// 该代币的元数据
    pub type TokenMetadataOf<T> = BoundedVec<u8, <T as Config>::MaxTokenMetadata>;
    /// 该类的信息
    pub type ClassInfoOf<T> = ClassInfo<
        <T as Config>::TotalId,
        <T as frame_system::Config>::AccountId,
        <T as Config>::ClassData,
        ClassMetadataOf<T>,
    >;
    /// 该代币的信息
    pub type TokenInfoOf<T> = TokenInfo<
        <T as frame_system::Config>::AccountId,
        <T as Config>::TokenData,
        TokenMetadataOf<T>,
    >;
    /// 通用代币数据
    pub type GenesisTokenData<T> = (
        // 代币的所有者
        <T as frame_system::Config>::AccountId, // Token owner
        // 代币的元数据
        Vec<u8>, // Token metadata
        // 代币的数据
        <T as Config>::TokenData,
        // 代币的ID
        <T as Config>::TokenId,
    );
    /// 通用代币集
    pub type GenesisTokens<T> = (
        // 代币所在类的所有者
        <T as frame_system::Config>::AccountId, // Token class owner
        // 代币所在类的元数据
        Vec<u8>, // Token class metadata
        // 类的数据
        <T as Config>::ClassData,
        // 通用代币集合
        Vec<GenesisTokenData<T>>, // Vector of tokens belonging to this class
    );

    /// Error for non-fungible-token module.
    /// NFT模块的错误
    #[pallet::error]
    pub enum Error<T> {
        /// No available class ID
        /// 没有可用的ClassId了
        NoAvailableClassId,

        /// Token(ClassId, TokenId) not found
        /// Token没有找到（ClassId下的TokenId不存在）
        TokenNotFound,
        /// Class not found
        /// 没有找到该类
        ClassNotFound,
        /// The operator is not the owner of the token and has no permission
        /// 权限不够
        NoPermission,
        /// Can not destroy class
        /// Total issuance is not 0
        /// 不能摧毁一个类，这个类的发行总量不为0
        CannotDestroyClass,
        /// Failed because the Maximum amount of metadata was exceeded
        /// 最大元数据溢出
        MaxMetadataExceeded,
    }

    /// Next available class ID.
    /// 下一个可用的类ID
    #[pallet::storage]
    #[pallet::getter(fn next_class_id)]
    pub type NextClassId<T: Config> = StorageValue<_, T::ClassId, ValueQuery>;
    /// Store class info.
    /// 存储类的信息
    ///
    /// Returns `None` if class info not set or removed.
    /// 返回 `None` 如果类信息没有设置或者被移除
    #[pallet::storage]
    #[pallet::getter(fn classes)]
    pub type Classes<T: Config> = StorageMap<_, Twox64Concat, T::ClassId, ClassInfoOf<T>>;

    /// Store token info.
    /// 存储代币信息
    ///
    /// Returns `None` if token info not set or removed.
    /// 返回 `None` 如果代币信息没有设置或者被移除
    #[pallet::storage]
    #[pallet::getter(fn tokens)]
    pub type Tokens<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::ClassId, Twox64Concat, T::TokenId, TokenInfoOf<T>>;

    /// Token existence check by owner and class ID.
    /// 通过所有者和类的ID检查代币是否存在
    #[pallet::storage]
    pub type TokensByOwner<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>, // owner
            NMapKey<Blake2_128Concat, T::ClassId>,
            NMapKey<Blake2_128Concat, T::TokenId>,
        ),
        (),
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub tokens: Vec<GenesisTokens<T>>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig { tokens: vec![] }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.tokens.iter().for_each(|token_class| {
                let class_id = Pallet::<T>::create_class(
                    &token_class.0,
                    token_class.1.to_vec(),
                    token_class.2.clone(),
                )
                .expect("Create class cannot fail while building genesis");
                for (account_id, token_metadata, token_data, token_id) in &token_class.3 {
                    Pallet::<T>::mint(
                        account_id,
                        (class_id, *token_id),
                        token_metadata.to_vec(),
                        token_data.clone(),
                    )
                    .expect("Token mint cannot fail during genesis");
                }
            })
        }
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {
    /// Create NFT(non fungible token) class
    /// 创建一个NFT类
    pub fn create_class(
        owner: &T::AccountId,
        metadata: Vec<u8>,
        data: T::ClassData,
    ) -> Result<T::ClassId, DispatchError> {
        let bounded_metadata: BoundedVec<u8, T::MaxClassMetadata> = metadata
            .try_into()
            .map_err(|_| Error::<T>::MaxMetadataExceeded)?;

        let class_id = NextClassId::<T>::try_mutate(|id| -> Result<T::ClassId, DispatchError> {
            let current_id = *id;
            *id = id
                .checked_add(&One::one())
                .ok_or(Error::<T>::NoAvailableClassId)?;
            Ok(current_id)
        })?;

        let info = ClassInfo {
            metadata: bounded_metadata,
            total_issuance: Default::default(),
            owner: owner.clone(),
            data,
        };
        Classes::<T>::insert(class_id, info);

        Ok(class_id)
    }

    /// Transfer NFT(non fungible token) from `from` account to `to` account
    /// 交易一个NFT
    /// 从 `from` 到 `to` 账户
    pub fn transfer(
        from: &T::AccountId,
        to: &T::AccountId,
        token: (T::ClassId, T::TokenId),
    ) -> DispatchResult {
        Tokens::<T>::try_mutate(token.0, token.1, |token_info| -> DispatchResult {
            let mut info = token_info.as_mut().ok_or(Error::<T>::TokenNotFound)?;
            ensure!(info.owner == *from, Error::<T>::NoPermission);
            if from == to {
                // no change needed
                return Ok(());
            }

            info.owner = to.clone();

            TokensByOwner::<T>::remove((from, token.0, token.1));
            TokensByOwner::<T>::insert((to, token.0, token.1), ());

            Ok(())
        })
    }

    /// Mint NFT(non fungible token) to `owner`
    /// 铸造一个NFT给 `owner`
    pub fn mint(
        owner: &T::AccountId,
        // 这里原来只是 class id
        // 但是因为域名本省具有自己的 token id
        // 因此铸造的时候参数需要自己制定 token id
        token: (T::ClassId, T::TokenId),
        metadata: Vec<u8>,
        data: T::TokenData,
    ) -> Result<(), DispatchError> {
        let (class_id, token_id) = token;

        let bounded_metadata: BoundedVec<u8, T::MaxTokenMetadata> = metadata
            .try_into()
            .map_err(|_| Error::<T>::MaxMetadataExceeded)?;

        Classes::<T>::try_mutate(class_id, |class_info| -> DispatchResult {
            let info = class_info.as_mut().ok_or(Error::<T>::ClassNotFound)?;
            info.total_issuance = info
                .total_issuance
                .checked_add(&One::one())
                .ok_or(ArithmeticError::Overflow)?;
            Ok(())
        })?;

        let token_info = TokenInfo {
            metadata: bounded_metadata,
            owner: owner.clone(),
            data,
        };
        Tokens::<T>::insert(class_id, token_id, token_info);
        TokensByOwner::<T>::insert((owner, class_id, token_id), ());

        Ok(())
    }

    /// Burn NFT(non fungible token) from `owner`
    /// 销毁 NFT 从 `owner`
    pub fn burn(owner: &T::AccountId, token: (T::ClassId, T::TokenId)) -> DispatchResult {
        Tokens::<T>::try_mutate_exists(token.0, token.1, |token_info| -> DispatchResult {
            let t = token_info.take().ok_or(Error::<T>::TokenNotFound)?;
            ensure!(t.owner == *owner, Error::<T>::NoPermission);

            Classes::<T>::try_mutate(token.0, |class_info| -> DispatchResult {
                let info = class_info.as_mut().ok_or(Error::<T>::ClassNotFound)?;
                info.total_issuance = info
                    .total_issuance
                    .checked_sub(&One::one())
                    .ok_or(ArithmeticError::Overflow)?;
                Ok(())
            })?;

            TokensByOwner::<T>::remove((owner, token.0, token.1));

            Ok(())
        })
    }

    /// Destroy NFT(non fungible token) class
    /// 销毁 NFT 类
    pub fn destroy_class(owner: &T::AccountId, class_id: T::ClassId) -> DispatchResult {
        Classes::<T>::try_mutate_exists(class_id, |class_info| -> DispatchResult {
            let info = class_info.take().ok_or(Error::<T>::ClassNotFound)?;
            ensure!(info.owner == *owner, Error::<T>::NoPermission);
            ensure!(
                info.total_issuance == Zero::zero(),
                Error::<T>::CannotDestroyClass
            );

            Tokens::<T>::remove_prefix(class_id, None);

            Ok(())
        })
    }
    /// 判断是否是所有者
    pub fn is_owner(account: &T::AccountId, token: (T::ClassId, T::TokenId)) -> bool {
        TokensByOwner::<T>::contains_key((account, token.0, token.1))
    }
}
