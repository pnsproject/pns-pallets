//! Benchmarking setup for pns-pallets
#![cfg(feature = "runtime-benchmarks")]

use crate::resolvers::{AddressKind, Call, Config, Pallet, TextKind};
use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::StaticLookup;
use sp_runtime::DispatchError;

benchmarks! {
    where_clause {
        where
        T: pns_registrar::origin::Config + pns_registrar::registrar::Config,
        T::DomainHash: From<T::Hash>,
    }

    set_account {
        let (owner,node) = get_cupnfishu_node::<T>()?;
    }: _(RawOrigin::Signed(owner),node.into(),AddressKind::Substrate,Default::default())

    set_text {
        let l in 0..10_000;
        let (owner,node) = get_cupnfishu_node::<T>()?;
    }: _(RawOrigin::Signed(owner), node.into(),TextKind::Email,sp_std::vec![7;l as usize])
}

fn get_cupnfishu_node<T>() -> Result<(T::AccountId, T::Hash), DispatchError>
where
    T: pns_registrar::registrar::Config + pns_registrar::origin::Config,
{
    let owner = create_caller::<T, T::Currency>(888);
    let owner_clone = owner.clone();
    pns_registrar::registrar::Pallet::<T>::register(
        RawOrigin::Signed(owner).into(),
        b"cupnfishuuu".to_vec(),
        account_to_source::<T>(owner_clone.clone()),
        T::MinRegistrationDuration::get(),
    )?;
    Ok((
        owner_clone,
        pns_registrar::traits::Label::<T::Hash>::new("cupnfishuuu".as_bytes())
            .unwrap()
            .0
            .encode_with_basenode(T::BaseNode::get()),
    ))
}

fn account_to_source<T: frame_system::Config>(
    account: T::AccountId,
) -> <T::Lookup as StaticLookup>::Source {
    <T::Lookup as StaticLookup>::unlookup(account)
}

fn create_caller<T, C>(idx: u32) -> T::AccountId
where
    T: frame_system::Config,
    C: frame_support::traits::Currency<T::AccountId>,
{
    let caller: T::AccountId = account("caller", idx, 996);
    let _ = C::deposit_creating(&caller, 888_888_888_u32.into());
    caller
}
