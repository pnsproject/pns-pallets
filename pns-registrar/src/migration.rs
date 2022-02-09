use core::marker::PhantomData;

use crate::{nft, origin, price_oracle, registry};

pub struct Initialize<T>(PhantomData<T>);

impl<T: registry::Config> Initialize<T> {
    pub fn initial_registry(official: T::AccountId, root_domain: T::Hash) {
        registry::Official::<T>::put(&official);

        let class_id = nft::Pallet::<T>::create_class(&official, Default::default(), ())
            .expect("Create class cannot fail while initialize");

        nft::Pallet::<T>::mint(
            &official,
            (class_id, root_domain),
            Default::default(),
            Default::default(),
        )
        .expect("Token mint cannot fail during initialize");
    }
}

impl<T: origin::Config> Initialize<T> {
    pub fn initial_origin(managers: Vec<T::AccountId>) {
        for manager in managers {
            origin::Origins::<T>::insert(manager, ());
        }
    }
}

type BalanceOf<T> = <<T as price_oracle::Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

impl<T: price_oracle::Config> Initialize<T> {
    /// (BasePrice or RentPrice) is dollar * exchange rate -> finale value
    pub fn initial_price_oracle(
        base_prices: Vec<BalanceOf<T>>,
        rent_prices: Vec<BalanceOf<T>>,
        init_rate: BalanceOf<T>,
    ) {
        <price_oracle::BasePrice<T>>::put(base_prices);
        <price_oracle::RentPrice<T>>::put(rent_prices);
        <price_oracle::ExchangeRate<T>>::put(init_rate);
    }
}
