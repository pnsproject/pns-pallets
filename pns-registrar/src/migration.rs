use core::marker::PhantomData;

use frame_support::dispatch::Weight;
use frame_support::traits::Get;
use sp_std::vec::Vec;

use crate::{nft, origin, price_oracle, registry};

pub struct Initialize<T>(PhantomData<T>);

impl<T: registry::Config> Initialize<T> {
    pub fn initial_registry(official: T::AccountId, root_domain: T::Hash) -> Weight {
        // writes 1
        registry::Official::<T>::put(&official);

        // writes 2
        let class_id = nft::Pallet::<T>::create_class(&official, Default::default(), ())
            .expect("Create class cannot fail while initialize");

        // writes 3
        nft::Pallet::<T>::mint(
            &official,
            (class_id, root_domain),
            Default::default(),
            Default::default(),
        )
        .expect("Token mint cannot fail during initialize");

        <T as frame_system::Config>::DbWeight::get().writes(6)
    }
}

impl<T: origin::Config> Initialize<T> {
    pub fn initial_origin(managers: Vec<T::AccountId>) -> Weight {
        let mut w = 0;
        for manager in managers {
            origin::Origins::<T>::insert(manager, ());
            w += 1;
        }
        <T as frame_system::Config>::DbWeight::get().writes(w)
    }
}

type BalanceOf<T> = <<T as price_oracle::Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

impl<T: price_oracle::Config> Initialize<T> {
    /// (`BasePrice` or `RentPrice`) is dollar * exchange rate -> finale value
    pub fn initial_price_oracle(
        base_prices: [BalanceOf<T>; 11],
        rent_prices: [BalanceOf<T>; 11],
        deposit_prices: [BalanceOf<T>; 11],
        init_rate: BalanceOf<T>,
    ) -> Weight {
        <price_oracle::BasePrice<T>>::put(base_prices);
        <price_oracle::RentPrice<T>>::put(rent_prices);
        <price_oracle::DepositPrice<T>>::put(deposit_prices);
        <price_oracle::ExchangeRate<T>>::put(init_rate);
        <T as frame_system::Config>::DbWeight::get().writes(3)
    }
}
