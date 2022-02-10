use codec::{Encode, FullCodec};
use core::fmt::Debug;
use frame_support::traits::Currency;

use sp_io::hashing::keccak_256;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize},
    DispatchError, DispatchResult,
};
use sp_std::vec::Vec;

pub trait Registrar {
    type Hash;
    type Balance;
    type AccountId;
    type Duration;
    fn check_expires_registrable(node: Self::Hash) -> DispatchResult;
    fn check_expires_renewable(node: Self::Hash) -> DispatchResult;
    fn check_expires_useable(node: Self::Hash) -> DispatchResult;
    fn clear_registrar_info(node: Self::Hash, owner: &Self::AccountId) -> DispatchResult;
    fn for_redeem_code(
        name: Vec<u8>,
        to: Self::AccountId,
        duration: Self::Duration,
        label: Label<Self::Hash>,
    ) -> DispatchResult;
    fn basenode() -> Self::Hash;
    // fn for_auction_set_expires(
    // 	node: Self::Hash,
    // 	deposit: Self::Balance,
    // 	register_fee: Self::Balance,
    // );
}

/// 登记表
pub trait Registry: NFT<Self::AccountId> {
    type AccountId;
    type Hash;

    fn mint_subname(
        node_owner: &Self::AccountId,
        node: Self::Hash,
        label_node: Self::Hash,
        to: Self::AccountId,
        capacity: u32,
        do_payments: impl FnOnce(Option<&Self::AccountId>) -> DispatchResult,
    ) -> DispatchResult;
    fn available(caller: &Self::AccountId, node: Self::Hash) -> DispatchResult;
    fn transfer(from: &Self::AccountId, to: &Self::AccountId, node: Self::Hash) -> DispatchResult;
    fn reclaimed(caller: &Self::AccountId, node: Self::Hash) -> DispatchResult;
}

// 客户
pub trait Customer<AccountId> {
    // 客户使用的货币
    type Currency: Currency<AccountId>;
}

pub trait PriceOracle {
    type Duration;
    type Balance;
    /// Returns the price to register or renew a name.
    /// * `name`: The name being registered or renewed.
    /// * `expires`: When the name presently expires (0 if this is a new registration).
    /// * `duration`: How long the name is being registered or extended for, in seconds.
    /// return The price of this renewal or registration, in wei.
    fn renew_price(name_len: usize, duration: Self::Duration) -> Option<Self::Balance>;
    fn registry_price(name_len: usize, duration: Self::Duration) -> Option<Self::Balance>;
    fn register_fee(name_len: usize) -> Option<Self::Balance>;
    fn deposit_fee(name_len: usize) -> Option<Self::Balance>;
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

pub struct Label<Hash> {
    pub node: Hash,
}
pub const LABEL_MAX_LEN: usize = 64;
pub const LABEL_MIN_LEN: usize = 3;

impl<Hash> Label<Hash>
where
    Hash: Default + AsMut<[u8]> + Encode + Clone,
{
    pub fn new(data: &[u8]) -> Option<(Self, usize)> {
        let label = core::str::from_utf8(data)
            .map(|label| label.to_ascii_lowercase())
            .ok()?;
        let label_len = label.len();
        if !(LABEL_MIN_LEN..=LABEL_MAX_LEN).contains(&label_len) {
            return None;
        }
        let mut flag = false;

        for res in label.bytes().enumerate() {
            match res {
                (i, c) if (i == 0 || i == label_len - 1) && !c.is_ascii_alphanumeric() => {
                    return None
                }
                (_, c) if flag && c == b'-' => return None,
                (_, c) if !flag && c == b'-' => flag = true,
                (_, c) if c.is_ascii_alphanumeric() => {
                    if flag {
                        flag = true;
                    }
                }
                _ => return None,
            }
        }
        let node = sp_core::convert_hash::<Hash, [u8; 32]>(&keccak_256(label.as_bytes()));
        Some((Self { node }, label_len))
    }

    pub fn encode_with_basenode(&self, basenode: Hash) -> Hash {
        let encoded = &(Hash::default(), basenode).encode();
        let hash_encoded = keccak_256(encoded);
        let encoded_again = &(hash_encoded, &self.node).encode();

        sp_core::convert_hash::<Hash, [u8; 32]>(&keccak_256(encoded_again))
    }

    pub fn encode_with_node(&self, node: Hash) -> Hash {
        let encoded = &(node, &self.node).encode();

        sp_core::convert_hash::<Hash, [u8; 32]>(&keccak_256(encoded))
    }
}

pub trait Available {
    fn is_anctionable(&self) -> bool;
    fn is_registrable(&self) -> bool;
}

impl Available for usize {
    fn is_anctionable(&self) -> bool {
        *self > LABEL_MIN_LEN && *self < 10
    }

    fn is_registrable(&self) -> bool {
        *self > 9
    }
}

pub trait IntoMoment<T> {
    type Moment;
    fn into_moment(self) -> Self::Moment;
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
