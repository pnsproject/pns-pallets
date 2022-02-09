pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::traits::ExchangeRate as ExchangeRateT;
    use frame_support::traits::{Currency, EnsureOrigin, Get};
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_runtime::traits::AtLeast32BitUnsigned;
    use sp_std::vec::Vec;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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

        #[pallet::constant]
        type MaximumLength: Get<u8>;

        type ExchangeRate: ExchangeRateT<Balance = BalanceOf<Self>>;

        type WeightInfo: WeightInfo;

        type ManagerOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type BasePrice<T: Config> = StorageValue<_, Vec<BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    pub type RentPrice<T: Config> = StorageValue<_, Vec<BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    pub type ExchangeRate<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub base_prices: Vec<BalanceOf<T>>,
        pub rent_prices: Vec<BalanceOf<T>>,
        pub init_rate: BalanceOf<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                base_prices: vec![],
                rent_prices: vec![],
                init_rate: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <BasePrice<T>>::put(&self.base_prices);
            <RentPrice<T>>::put(&self.rent_prices);
            <ExchangeRate<T>>::put(&self.init_rate);
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Base praice changed
        /// `[base_prices]`
        BasePriceChanged(Vec<BalanceOf<T>>),
        /// Rent price changed
        /// `[rent_prices]`
        RentPriceChanged(Vec<BalanceOf<T>>),
        ExchangeRateChanged(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        NoneValue,
        StorageOverflow,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::set_exchange_rate())]
        pub fn set_exchange_rate(
            origin: OriginFor<T>,
            exchange_rate: BalanceOf<T>,
        ) -> DispatchResult {
            let who = T::ManagerOrigin::ensure_origin(origin)?;

            <ExchangeRate<T>>::put(&exchange_rate);

            Self::deposit_event(Event::ExchangeRateChanged(who, exchange_rate));

            Ok(())
        }
        /// Internal root method.
        #[pallet::weight(T::WeightInfo::set_base_price(prices.len() as u32))]
        pub fn set_base_price(origin: OriginFor<T>, prices: Vec<BalanceOf<T>>) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            <BasePrice<T>>::put(&prices);

            Self::deposit_event(Event::BasePriceChanged(prices));

            Ok(())
        }
        /// Internal root method.
        #[pallet::weight(T::WeightInfo::set_rent_price(prices.len() as u32))]
        pub fn set_rent_price(origin: OriginFor<T>, prices: Vec<BalanceOf<T>>) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            <RentPrice<T>>::put(&prices);

            Self::deposit_event(Event::RentPriceChanged(prices));

            Ok(())
        }
    }
}
use crate::traits::{ExchangeRate as ExchangeRateT, PriceOracle};
use frame_support::pallet_prelude::Weight;
use sp_runtime::{
    traits::{CheckedDiv, CheckedMul},
    SaturatedConversion,
};

pub trait WeightInfo {
    fn set_exchange_rate() -> Weight;
    fn set_base_price(len: u32) -> Weight;
    fn set_rent_price(len: u32) -> Weight;
}

impl<T: Config> PriceOracle for Pallet<T> {
    type Duration = T::Moment;

    type Balance = BalanceOf<T>;

    // TODO: 更合理的押金计算方式
    fn deposit_fee(name_len: usize) -> Option<Self::Balance> {
        Self::register_fee(name_len).and_then(|register_fee| {
            register_fee.checked_div(&Self::Balance::saturated_from(2_u128))
        })
    }

    fn register_fee(name_len: usize) -> Option<Self::Balance> {
        let base_prices = BasePrice::<T>::get();
        let prices_len = base_prices.len();
        let len = if name_len < prices_len {
            name_len
        } else {
            prices_len
        };
        let exchange_rate = T::ExchangeRate::get_exchange_rate();

        base_prices[len - 1].checked_mul(&exchange_rate)
    }

    fn registry_price(name_len: usize, duration: Self::Duration) -> Option<Self::Balance> {
        let register_price = Self::register_fee(name_len)?;
        let rent_price = Self::renew_price(name_len, duration)?;

        Some(register_price + rent_price)
    }
    fn renew_price(name_len: usize, duration: Self::Duration) -> Option<Self::Balance> {
        let rent_prices = RentPrice::<T>::get();
        let prices_len = rent_prices.len();
        let len = if name_len < prices_len {
            name_len
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
