//! # Price Oracle
//!
//! This module is responsible for providing a price list
//! that can be set dynamically, but it is not intelligent
//! and the base price can only be set manually by the manager.
//! (A more intelligent approach, such as an off-chain worker,
//!  is being considered)
//!
//! ## Introduction
//!
//! This module is used to calculate the parameters required
//! for the base price of domain name registrations and auctions.
//!
//! ### Module functions
//!
//! - `set_exchange_rate` - sets the local rate
//! - `set_base_price` - sets the base price
//! - `set_rent_price` - sets the price used for time growth
//!
//! All the above methods require manager privileges in `pnsOrigin`.
//!
//! Note that the `trait` of `ExchangeRate` is to conveniently follow
//! if the parallel chain itself provides price oracle related functions,
//! and can be directly replaced.
//!
pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::traits::ExchangeRate as ExchangeRateT;
    use frame_support::traits::{Currency, EnsureOrigin};
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_runtime::traits::AtLeast32BitUnsigned;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type Currency: Currency<Self::AccountId>;

        type Moment: Clone
            + Copy
            + Decode
            + Encode
            + Eq
            + PartialEq
            + core::fmt::Debug
            + Default
            + TypeInfo
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize;

        type ExchangeRate: ExchangeRateT<Balance = BalanceOf<Self>>;

        type WeightInfo: WeightInfo;

        type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // TODO: 只有11档价格，并且档数的选择与域名长度相关
    #[pallet::storage]
    pub type BasePrice<T: Config> = StorageValue<_, [BalanceOf<T>; 11], ValueQuery>;

    #[pallet::storage]
    pub type RentPrice<T: Config> = StorageValue<_, [BalanceOf<T>; 11], ValueQuery>;

    #[pallet::storage]
    pub type DepositPrice<T: Config> = StorageValue<_, [BalanceOf<T>; 11], ValueQuery>;

    #[pallet::storage]
    pub type ExchangeRate<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub base_prices: [BalanceOf<T>; 11],
        pub rent_prices: [BalanceOf<T>; 11],
        pub deposit_prices: [BalanceOf<T>; 11],
        pub init_rate: BalanceOf<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                base_prices: [Default::default(); 11],
                rent_prices: [Default::default(); 11],
                deposit_prices: [Default::default(); 11],
                init_rate: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <BasePrice<T>>::put(self.base_prices);
            <RentPrice<T>>::put(self.rent_prices);
            <DepositPrice<T>>::put(self.deposit_prices);
            <ExchangeRate<T>>::put(self.init_rate);
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Base praice changed
        /// `[base_prices]`
        BasePriceChanged([BalanceOf<T>; 11]),
        /// Rent price changed
        /// `[rent_prices]`
        RentPriceChanged([BalanceOf<T>; 11]),
        /// Deposit price changed
        /// `[deposit_prices]`
        DepositPriceChanged([BalanceOf<T>; 11]),
        /// Exchange rate changed
        /// `[who, rate]`
        ExchangeRateChanged(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_exchange_rate())]
        pub fn set_exchange_rate(
            origin: OriginFor<T>,
            exchange_rate: BalanceOf<T>,
        ) -> DispatchResult {
            let who = T::ManagerOrigin::ensure_origin(origin)?;

            <ExchangeRate<T>>::put(exchange_rate);

            Self::deposit_event(Event::ExchangeRateChanged(who, exchange_rate));

            Ok(())
        }
        /// Internal root method.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_base_price())]
        pub fn set_base_price(origin: OriginFor<T>, prices: [BalanceOf<T>; 11]) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            <BasePrice<T>>::put(prices);

            Self::deposit_event(Event::BasePriceChanged(prices));

            Ok(())
        }
        /// Internal root method.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_rent_price())]
        pub fn set_rent_price(origin: OriginFor<T>, prices: [BalanceOf<T>; 11]) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            <RentPrice<T>>::put(prices);

            Self::deposit_event(Event::RentPriceChanged(prices));

            Ok(())
        }
        /// Internal root method.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_deposit_price())]
        pub fn set_deposit_price(
            origin: OriginFor<T>,
            prices: [BalanceOf<T>; 11],
        ) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            <DepositPrice<T>>::put(prices);

            Self::deposit_event(Event::DepositPriceChanged(prices));

            Ok(())
        }
    }
}
use crate::traits::{ExchangeRate as ExchangeRateT, PriceOracle};
use frame_support::pallet_prelude::Weight;
use sp_runtime::{traits::CheckedMul, SaturatedConversion};

pub trait WeightInfo {
    fn set_exchange_rate() -> Weight;
    fn set_base_price() -> Weight;
    fn set_rent_price() -> Weight;
    fn set_deposit_price() -> Weight;
}

impl<T: Config> PriceOracle for Pallet<T> {
    type Moment = T::Moment;

    type Balance = BalanceOf<T>;

    fn deposit_fee(name_len: usize) -> Option<Self::Balance> {
        let deposit_prices = DepositPrice::<T>::get();
        let prices_len = deposit_prices.len();
        let len = if name_len < prices_len {
            name_len.max(1)
        } else {
            prices_len
        };
        let exchange_rate = T::ExchangeRate::get_exchange_rate();
        deposit_prices[len - 1].checked_mul(&exchange_rate)
    }

    fn registration_fee(name_len: usize) -> Option<Self::Balance> {
        let base_prices = BasePrice::<T>::get();
        let prices_len = base_prices.len();
        let len = if name_len < prices_len {
            name_len.max(1)
        } else {
            prices_len
        };
        let exchange_rate = T::ExchangeRate::get_exchange_rate();

        base_prices[len - 1].checked_mul(&exchange_rate)
    }

    fn register_fee(name_len: usize, duration: Self::Moment) -> Option<Self::Balance> {
        let register_price = Self::registration_fee(name_len)?;
        let rent_price = Self::renew_fee(name_len, duration)?;

        Some(register_price + rent_price)
    }
    fn renew_fee(name_len: usize, duration: Self::Moment) -> Option<Self::Balance> {
        let rent_prices = RentPrice::<T>::get();
        let prices_len = rent_prices.len();
        let len = if name_len < prices_len {
            name_len.max(1)
        } else {
            prices_len
        };
        let duration = duration.saturated_into::<u128>();
        let rent_price = (rent_prices[len - 1].checked_mul(&T::ExchangeRate::get_exchange_rate()))?
            .saturated_into::<u128>();

        rent_price
            .checked_mul(duration)
            .map(|res| res.saturated_into::<Self::Balance>())
    }
}

impl<T: Config> ExchangeRateT for Pallet<T> {
    type Balance = BalanceOf<T>;

    fn get_exchange_rate() -> Self::Balance {
        ExchangeRate::<T>::get()
    }
}

impl WeightInfo for () {
    fn set_exchange_rate() -> Weight {
        Weight::zero()
    }

    fn set_base_price() -> Weight {
        Weight::zero()
    }

    fn set_rent_price() -> Weight {
        Weight::zero()
    }

    fn set_deposit_price() -> Weight {
        Weight::zero()
    }
}
