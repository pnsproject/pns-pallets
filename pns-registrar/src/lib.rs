#![cfg_attr(not(feature = "std"), no_std)]

//pub mod auction;
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

#[cfg(any(test, feature = "runtime-benchmarks"))]
#[path = "../../pns-resolvers/src/resolvers.rs"]
pub mod resolvers;
