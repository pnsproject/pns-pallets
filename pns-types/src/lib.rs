#![cfg_attr(not(feature = "std"), no_std)]

pub mod ddns;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, PartialEq, Eq, RuntimeDebug, Clone, TypeInfo, MaxEncodedLen)]
pub struct RegistrarInfo<Moment, Balance> {
    /// 到期的时间
    pub expire: Moment,
    /// 可创建的子域名容量
    pub capacity: u32,
    /// 押金
    pub deposit: Balance,
    /// 注册费
    pub register_fee: Balance,
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, PartialEq, Eq, RuntimeDebug, Clone, TypeInfo, MaxEncodedLen)]
pub enum DomainTracing {
    RuntimeOrigin(DomainHash),
    Root,
}

/// 域名记录
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, PartialEq, Eq, RuntimeDebug, Clone, Default, TypeInfo)]
pub struct Record {
    pub children: u32,
}

pub type DomainHash = sp_core::H256;

#[test]
fn test() {
    println!("hello")
}
