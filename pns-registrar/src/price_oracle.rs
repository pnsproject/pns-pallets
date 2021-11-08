use std::ops::Mul;

pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::traits::{Currency, Get};
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Currency: Currency<Self::AccountId>;

        #[pallet::constant]
        type MaximumLength: Get<u8>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type BasePrice<T: Config> = StorageValue<_, Vec<BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    pub type RentPrice<T: Config> = StorageValue<_, Vec<BalanceOf<T>>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub base_prices: Vec<BalanceOf<T>>,
        pub rent_prices: Vec<BalanceOf<T>>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                base_prices: vec![],
                rent_prices: vec![],
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <BasePrice<T>>::put(&self.base_prices);
            <RentPrice<T>>::put(&self.rent_prices);
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// `[base_prices]`
        BasePriceChanged(Vec<BalanceOf<T>>),
        /// `[rent_prices]`
        RentPriceChanged(Vec<BalanceOf<T>>),
    }

    #[pallet::error]
    pub enum Error<T> {
        NoneValue,
        StorageOverflow,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::set_price())]
        pub fn set_base_price(origin: OriginFor<T>, prices: Vec<BalanceOf<T>>) -> DispatchResult {
            ensure_root(origin)?;

            <BasePrice<T>>::put(&prices);

            Self::deposit_event(Event::BasePriceChanged(prices));

            Ok(())
        }
        #[pallet::weight(T::WeightInfo::set_price())]
        pub fn set_rent_price(origin: OriginFor<T>, prices: Vec<BalanceOf<T>>) -> DispatchResult {
            ensure_root(origin)?;

            <RentPrice<T>>::put(&prices);

            Self::deposit_event(Event::RentPriceChanged(prices));

            Ok(())
        }
    }
}
use crate::traits::PriceOracle;
use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
    fn set_price() -> Weight;
}

impl<T: Config> PriceOracle for Pallet<T> {
    type Duration = T::BlockNumber;

    type Balance = BalanceOf<T>;

    fn renew_price(name_len: usize, duration: Self::Duration) -> Self::Balance {
        use sp_runtime::SaturatedConversion;
        let rent_prices = RentPrice::<T>::get();
        let prices_len = rent_prices.len();
        let len = if name_len > prices_len {
            name_len
        } else {
            prices_len
        };
        let duration = duration.saturated_into::<u128>();
        let rent_price = rent_prices[len - 1].saturated_into::<u128>();
        Self::Balance::saturated_from(rent_price.mul(duration))
    }

    fn registry_price(name_len: usize, duration: Self::Duration) -> Self::Balance {
        let base_prices = BasePrice::<T>::get();
        let prices_len = base_prices.len();
        let len = if name_len > prices_len {
            name_len
        } else {
            prices_len
        };
        let rent_price = Self::renew_price(name_len, duration);

        base_prices[len - 1] + rent_price
    }

    fn register_fee(name_len: usize) -> Self::Balance {
        let base_prices = BasePrice::<T>::get();
        let prices_len = base_prices.len();
        let len = if name_len > prices_len {
            name_len
        } else {
            prices_len
        };
        base_prices[len - 1]
    }
}
