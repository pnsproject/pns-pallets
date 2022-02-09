#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
extern crate alloc;

//pub mod auction;
pub mod migration;
pub mod nft;
pub mod origin;
pub mod price_oracle;
pub mod redeem_code;
pub mod registrar;
pub mod registry;
pub mod traits;

#[cfg(test)]
pub mod mock;

#[cfg(test)]
pub(crate) mod tests;

#[cfg(any(test, feature = "runtime-benchmarks"))]
mod benchmarks;
