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
mod mock;
#[cfg(test)]
mod tests;
