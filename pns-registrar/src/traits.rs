use codec::{Encode, FullCodec};
use core::fmt::Debug;
use frame_support::traits::Currency;
use pns_types::DomainHash;

use sp_io::hashing::keccak_256;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize},
    DispatchError, DispatchResult,
};
use sp_std::vec::Vec;

pub trait Registrar {
    type Balance;
    type AccountId;
    type Moment;
    fn check_expires_registrable(node: DomainHash) -> DispatchResult;
    fn check_expires_renewable(node: DomainHash) -> DispatchResult;
    fn check_expires_useable(node: DomainHash) -> DispatchResult;
    fn clear_registrar_info(node: DomainHash, owner: &Self::AccountId) -> DispatchResult;
    fn for_redeem_code(
        name: Vec<u8>,
        to: Self::AccountId,
        duration: Self::Moment,
        label: Label,
    ) -> DispatchResult;
    fn basenode() -> DomainHash;
    // fn for_auction_set_expires(
    // 	node: DomainHash,
    // 	deposit: Self::Balance,
    // 	register_fee: Self::Balance,
    // );
}

/// 登记表
pub trait Registry: NFT<Self::AccountId> {
    type AccountId;

    fn mint_subname(
        node_owner: &Self::AccountId,
        node: DomainHash,
        label_node: DomainHash,
        to: Self::AccountId,
        capacity: u32,
        do_payments: impl FnOnce(Option<&Self::AccountId>) -> DispatchResult,
    ) -> DispatchResult;
    fn available(caller: &Self::AccountId, node: DomainHash) -> DispatchResult;
    fn transfer(from: &Self::AccountId, to: &Self::AccountId, node: DomainHash) -> DispatchResult;
}

// 客户
pub trait Customer<AccountId> {
    // 客户使用的货币
    type Currency: Currency<AccountId>;
}

pub trait PriceOracle {
    type Moment;
    type Balance;
    /// Returns the price to register or renew a name.
    /// * `name`: The name being registered or renewed.
    /// * `expires`: When the name presently expires (0 if this is a new registration).
    /// * `duration`: How long the name is being registered or extended for, in seconds.
    /// return The price of this renewal or registration, in wei.
    fn renew_fee(name_len: usize, duration: Self::Moment) -> Option<Self::Balance>;
    fn register_fee(name_len: usize, duration: Self::Moment) -> Option<Self::Balance>;
    fn deposit_fee(name_len: usize) -> Option<Self::Balance>;
    fn registration_fee(name_len: usize) -> Option<Self::Balance>;
}

/// Abstraction over a non-fungible token system.
#[allow(clippy::upper_case_acronyms)]
pub trait NFT<AccountId> {
    /// The NFT class identifier.
    type ClassId: Default + Copy;

    /// The NFT token identifier.
    type TokenId: Default + Copy;

    /// The balance of account.
    type Balance: AtLeast32BitUnsigned
        + FullCodec
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + Default;

    /// The number of NFTs assigned to `who`.
    fn balance(who: &AccountId) -> Self::Balance;

    /// The owner of the given token ID. Returns `None` if the token does not
    /// exist.
    fn owner(token: (Self::ClassId, Self::TokenId)) -> Option<AccountId>;

    /// Transfer the given token ID from one account to another.
    fn transfer(
        from: &AccountId,
        to: &AccountId,
        token: (Self::ClassId, Self::TokenId),
    ) -> DispatchResult;
}

pub struct Label {
    pub node: DomainHash,
}
pub const LABEL_MAX_LEN: usize = 63;
pub const LABEL_MIN_LEN: usize = 3;
pub const MIN_REGISTRABLE_LEN: usize = 3;

impl Label {
    pub fn new(data: &[u8]) -> Option<Self> {
        check_label(data)?;

        let node = DomainHash::from(keccak_256(data));
        Some(Self { node })
    }
    pub fn new_basenode(data: &[u8]) -> Option<Self> {
        check_label(data)?;

        let node = DomainHash::from(keccak_256(data));

        let encoded = &(DomainHash::default(), node).encode();
        let hash_encoded = keccak_256(encoded);

        Some(Self {
            node: DomainHash::from(hash_encoded),
        })
    }

    pub fn encode_with_name(&self, data: &[u8]) -> Option<Self> {
        let node = Self::new(data)?;
        Some(Label {
            node: self.encode_with_node(&node.node),
        })
    }

    pub fn encode_with_basename(&self, data: &[u8]) -> Option<Self> {
        let node = Self::new(data)?;
        Some(Label {
            node: self.encode_with_baselabel(&node.node),
        })
    }
    pub fn new_with_len(data: &[u8]) -> Option<(Self, usize)> {
        check_label(data)?;

        let node = DomainHash::from(keccak_256(data));
        Some((Self { node }, data.len()))
    }

    pub fn encode_with_baselabel(&self, baselabel: &DomainHash) -> DomainHash {
        let basenode = Self::basenode(baselabel);
        let encoded_again = &(basenode, &self.node).encode();

        DomainHash::from(keccak_256(encoded_again))
    }

    pub fn basenode(baselabel: &DomainHash) -> DomainHash {
        let encoded = &(DomainHash::default(), baselabel).encode();
        let hash_encoded = keccak_256(encoded);
        DomainHash::from(hash_encoded)
    }

    pub fn to_basenode(&self) -> DomainHash {
        Self::basenode(&self.node)
    }

    pub fn encode_with_node(&self, node: &DomainHash) -> DomainHash {
        let encoded = &(node, &self.node).encode();

        DomainHash::from(keccak_256(encoded))
    }
}
// TODO: (暂不支持中文域名)
// 域名不区分大小写和简繁体。
// 域名的合法长度为1~63个字符（域名主体，不包括后缀）。
// 英文域名合法字符为a-z、0-9、短划线（-）。
// （ 说明 短划线（-）不能出现在开头和结尾以及在第三和第四字符位置。）
// 中文域名除英文域名合法字符外，必须含有至少一个汉字（简体或繁体），计算中文域名字符长度以转换后的punycode码为准。
// 不支持xn—开头的请求参数（punycode码），请以中文域名作为请求参数。
pub fn check_label(label: &[u8]) -> Option<()> {
    let label = core::str::from_utf8(label)
        .map(|label| label.to_ascii_lowercase())
        .ok()?;

    if !(LABEL_MIN_LEN..=LABEL_MAX_LEN).contains(&label.len()) {
        return None;
    }

    let label_chars = label.chars().collect::<Vec<_>>();

    match label_chars.as_slice() {
        [first, middle @ .., last]
            if first.is_ascii_alphanumeric() && last.is_ascii_alphanumeric() =>
        {
            for (i, &c) in middle.iter().enumerate() {
                match c {
                    c if c.is_ascii_alphanumeric() => continue,
                    c if c == '-' => {
                        if i == 1 || i == 2 {
                            return None;
                        }
                        continue;
                    }
                    _ => return None,
                }
            }
        }
        _ => return None,
    }

    Some(())
}
pub trait Available {
    fn is_anctionable(&self) -> bool;
    fn is_registrable(&self) -> bool;
}

impl Available for usize {
    fn is_anctionable(&self) -> bool {
        *self > LABEL_MIN_LEN && *self < MIN_REGISTRABLE_LEN
    }

    fn is_registrable(&self) -> bool {
        *self >= MIN_REGISTRABLE_LEN
    }
}

pub trait ExchangeRate {
    type Balance;
    /// 1 USD to balance
    fn get_exchange_rate() -> Self::Balance;
}

pub trait Official {
    type AccountId;

    fn get_official_account() -> Result<Self::AccountId, DispatchError>;
}

pub trait IsRegistrarOpen {
    fn is_open() -> bool;
}
