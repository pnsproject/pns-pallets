#![cfg_attr(not(feature = "std"), no_std)]

pub mod resolvers;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarks;
