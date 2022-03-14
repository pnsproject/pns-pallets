/*!
# Resolvers
This module provides functionality for domain name resolution. Most of these interfaces are interfaces provided for subsequent cooperation with wallets.

### Module functions
- `set_account` - sets the account resolve, which requires the domain to be available relative to that user (ownership of the domain, the domain is not expired)
- `set_text` - set text parsing, same requirements as above
!*/

use codec::MaxEncodedLen;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use serde::{Deserialize, Serialize};
    use sp_runtime::traits::{CheckEqual, MaybeDisplay, MaybeMallocSizeOf, SimpleBitOps};
    use sp_std::vec;
    use sp_std::vec::Vec;

    use super::RegistryChecker;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type WeightInfo: WeightInfo;
        /// 域名使用权限检查接口
        type RegistryChecker: RegistryChecker<Hash = Self::DomainHash, AccountId = Self::AccountId>;
        /// 域名哈希
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
    pub enum Address<Id> {
        /// substrate 地址
        Substrate([u8; 32]),
        /// 比特币地址
        Bitcoin([u8; 25]),
        /// 以太坊地址
        Ethereum([u8; 20]),
        /// 当前链定制的特定地址，通常和Substrate地址一致
        Id(Id),
    }
    /// account_id mapping
    #[pallet::storage]
    pub type Accounts<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::DomainHash, Twox64Concat, Address<T::AccountId>, ()>;
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
        /// 电子邮箱
        Email,
        /// 地址
        Url,
        /// 头像
        Avatar,
        /// 描述
        Description,
        /// 注意
        Notice,
        /// 关键字
        Keywords,
        /// 推特
        Twitter,
        /// Github地址
        Github,
        /// IPFS地址
        Ipfs,
    }
    /// text mapping
    #[pallet::storage]
    pub type Texts<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::DomainHash,
        Twox64Concat,
        TextKind,
        Content,
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// vec![ `node` , `address` ]
        pub accounts: Vec<(T::DomainHash, Address<T::AccountId>)>,
        /// vec![ `node` , `text_kind` , `text` ]
        pub texts: Vec<(T::DomainHash, TextKind, Content)>,
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
            for (node, address_kind) in self.accounts.iter().cloned() {
                Accounts::<T>::insert(node, address_kind, ());
            }

            for (node, text_kind, text) in self.texts.iter().cloned() {
                Texts::<T>::insert(node, text_kind, text);
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddressChanged {
            node: T::DomainHash,
            address: Address<T::AccountId>,
        },
        TextsChanged {
            node: T::DomainHash,
            kind: TextKind,
            content: Content,
        },
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
        /// 设置账户
        #[pallet::weight(T::WeightInfo::set_account())]
        pub fn set_account(
            origin: OriginFor<T>,
            // 传入的域名哈希
            node: T::DomainHash,
            // 要设置的地址
            address: Address<T::AccountId>,
        ) -> DispatchResult {
            // 验证调用者是否签名
            let who = ensure_signed(origin)?;
            // 确定域名修改权限
            ensure!(
                T::RegistryChecker::check_node_useable(node, &who),
                Error::<T>::InvalidPermission
            );
            // 插入账户
            Accounts::<T>::insert(node, &address, ());
            // 保存账户更改的事件
            Self::deposit_event(Event::<T>::AddressChanged { node, address });

            Ok(())
        }
        /// 设置文本信息
        #[pallet::weight(T::WeightInfo::set_text(content.0.len() as u32))]
        pub fn set_text(
            origin: OriginFor<T>,
            // 传入的域名哈希
            node: T::DomainHash,
            // 文本的类型
            kind: TextKind,
            // 文本的内容
            content: Content,
        ) -> DispatchResult {
            // 确保是已经签名的账户
            let who = ensure_signed(origin)?;
            // 确保域名的设置权限
            ensure!(
                T::RegistryChecker::check_node_useable(node, &who),
                Error::<T>::InvalidPermission
            );
            // 插入文本
            Texts::<T>::insert(node, &kind, &content);
            // 保存插入文本的事件
            Self::deposit_event(Event::<T>::TextsChanged {
                node,
                kind,
                content,
            });

            Ok(())
        }
    }
}

use frame_support::dispatch::Weight;
use sp_std::vec::Vec;

pub trait WeightInfo {
    fn set_text(content_len: u32) -> Weight;
    fn set_account() -> Weight;
}

pub trait RegistryChecker {
    type Hash;
    type AccountId;
    // 检查节点是否可用的接口
    fn check_node_useable(node: Self::Hash, owner: &Self::AccountId) -> bool;
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
    Default,
)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct Content(pub Vec<u8>);

impl MaxEncodedLen for Content {
    fn max_encoded_len() -> usize {
        1024
    }
}

impl From<Vec<u8>> for Content {
    fn from(inner: Vec<u8>) -> Self {
        Content(inner)
    }
}

impl WeightInfo for () {
    fn set_text(_content_len: u32) -> Weight {
        0
    }

    fn set_account() -> Weight {
        0
    }
}
