//! Benchmarking setup for pns-pallets
#![cfg(feature = "runtime-benchmarks")]

use frame_benchmarking::account;
use sp_runtime::traits::StaticLookup;
use sp_std::vec::Vec;
pub const SEED: u32 = 996;

pub fn get_rand_name(len: usize) -> Vec<u8> {
    let mut name = "cupnfishxx".to_ascii_lowercase();
    for _ in 10..len {
        name.push_str("x");
    }
    name.into_bytes()
}

pub fn name_to_node<H>(name: Vec<u8>, basenode: H) -> H
where
    H: Default + AsMut<[u8]> + codec::Encode + Clone,
{
    let (label, _len) = crate::traits::Label::<H>::new(&name).unwrap();
    label.encode_with_basenode(basenode)
}

pub fn account_to_source<T: frame_system::Config>(
    account: T::AccountId,
) -> <T::Lookup as StaticLookup>::Source {
    <T::Lookup as StaticLookup>::unlookup(account)
}

pub fn get_manager<T: crate::origin::Config>() -> T::AccountId {
    crate::origin::Origins::<T>::iter_keys().next().unwrap()
}

pub fn create_caller<T, C>(idx: u32) -> T::AccountId
where
    T: frame_system::Config,
    C: frame_support::traits::Currency<T::AccountId>,
{
    let caller: T::AccountId = account("caller", idx, SEED);
    let _ = C::deposit_creating(&caller, 888_888_888_u32.into());
    caller
}

pub fn poor_account<T: frame_system::Config>(idx: u32) -> T::AccountId {
    let caller: T::AccountId = account("caller", idx, SEED);
    caller
}

mod registry {
    use super::{account_to_source, get_manager};
    #[cfg(test)]
    use crate::mock::Test;
    use crate::{
        registry::{Call, Config, Pallet},
        traits::{Label, Registrar},
    };
    use frame_benchmarking::Zero;
    use frame_benchmarking::{account, benchmarks};
    use frame_system::RawOrigin;
    use sp_runtime::DispatchError;

    const SEED: u32 = 996;

    fn get_account_and_node<T: Config>(
        name: &'static str,
        index: u32,
    ) -> Result<(T::AccountId, T::Hash), DispatchError> {
        let owner = account::<T::AccountId>(name, index, SEED);
        let label = Label::<T::Hash>::new(alloc::format!("{name}{index}").as_bytes())
            .unwrap()
            .0;
        let class_id = T::ClassId::zero();
        let node = label.encode_with_basenode(T::Registrar::basenode());

        crate::nft::Pallet::<T>::mint(
            &owner,
            (class_id, node),
            Default::default(),
            Default::default(),
        )?;
        use crate::registry::DomainTracing;
        use crate::registry::Origin;
        if let Some(_) = Origin::<T>::get(T::Registrar::basenode()) {
            panic!("Unexpected arm");
        } else {
            Pallet::<T>::add_children(T::Registrar::basenode(), class_id)?;

            Origin::<T>::insert(node, DomainTracing::Root);
        }

        Ok((owner, node))
    }

    benchmarks! {
        where_clause {
            where
            T: crate::origin::Config,
        }
        approval_for_all_true {
            let caller = account::<T::AccountId>("caller",0,SEED);
            let operator = account::<T::AccountId>("operator",1,SEED);
            let approved = true;
        }: approval_for_all(RawOrigin::Signed(caller.clone()), account_to_source::<T>(operator.clone()),approved)
        verify {
            assert_eq!(crate::registry::OperatorApprovals::<T>::contains_key(caller,operator), approved);
        }
        approval_for_all_false {
            let caller = account::<T::AccountId>("caller",0,SEED);
            let operator = account::<T::AccountId>("operator",1,SEED);
            let approved = false;
            Pallet::<T>::approval_for_all(RawOrigin::Signed(caller.clone()).into(), account_to_source::<T>(operator.clone()),!approved)?;
        }: approval_for_all(RawOrigin::Signed(caller.clone()), account_to_source::<T>(operator.clone()),approved)
        verify {
            assert_eq!(crate::registry::OperatorApprovals::<T>::contains_key(caller,operator), approved);
        }
        set_resolver {
            let (owner,node) = get_account_and_node::<T>("caller",0)?;
        }: _(RawOrigin::Signed(owner), node,T::ResolverId::default())
        verify {
            assert_eq!(crate::registry::Resolver::<T>::get(node), T::ResolverId::default());
        }
        destroy {
            let (owner,node) = get_account_and_node::<T>("caller",3)?;
        }: _(RawOrigin::Signed(owner), node)
        verify {
            assert!(!crate::nft::Tokens::<T>::contains_key(T::ClassId::zero(),node));
        }
        set_official {
            let official = account::<T::AccountId>("official",567,SEED);
        }: _(RawOrigin::Signed(get_manager::<T>()), official.clone())
        verify {
            assert_eq!(crate::registry::Official::<T>::get(), official);
        }
        approve_true {
            let (owner,node) = get_account_and_node::<T>("owner",567)?;
            let to = account::<T::AccountId>("to",996,SEED);
        }: approve(RawOrigin::Signed(owner), to.clone(),node,true)
        verify {
            assert!(crate::registry::TokenApprovals::<T>::contains_key(node,to.clone()));
        }
        approve_false {
            let (owner,node) = get_account_and_node::<T>("owner",567)?;
            let to = account::<T::AccountId>("to",996,SEED);
            crate::registry::Pallet::<T>::approve(RawOrigin::Signed(owner.clone()).into(), to.clone(),node,true)?;
        }: approve(RawOrigin::Signed(owner), to.clone(),node,false)
        verify {
            assert!(!crate::registry::TokenApprovals::<T>::contains_key(node,to));
        }
        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod registrar {
    use super::{account_to_source, create_caller, get_manager, get_rand_name, name_to_node};
    #[cfg(test)]
    use crate::mock::Test;
    use crate::{
        registrar::{Call, Config, Pallet},
        traits::{Label, Registrar, LABEL_MAX_LEN, LABEL_MIN_LEN},
    };
    use frame_benchmarking::benchmarks;
    use frame_support::traits::{Currency, Get};
    use frame_system::RawOrigin;
    use sp_std::vec::Vec;

    fn get_rand_node<T: Config>(seed: u32) -> T::Hash {
        let name = alloc::format!("rand{seed}");
        let label = Label::<T::Hash>::new(name.as_bytes()).unwrap().0;
        label.node
    }

    fn get_subname(len: usize) -> Vec<u8> {
        let mut name = "abc".to_ascii_lowercase();
        for _ in LABEL_MIN_LEN..len {
            name.push_str("x");
        }
        name.into_bytes()
    }

    fn get_subhash<T: Config>(subname: &[u8], node: T::Hash) -> T::Hash {
        let (label, _len) = Label::new(subname).unwrap();
        label.encode_with_node(node)
    }

    benchmarks! {
        where_clause {
            where
            T: crate::origin::Config,
        }
        add_reserved {
            let node = get_rand_node::<T>(567);
            let manager = get_manager::<T>();
        }:_(RawOrigin::Signed(manager), node)
        verify {
            assert!(crate::registrar::ReservedList::<T>::contains_key(node));
        }
        remove_reserved {
            let node = get_rand_node::<T>(567);
            let manager = get_manager::<T>();

            Pallet::<T>::add_reserved(RawOrigin::Signed(get_manager::<T>()).into(), node)?;
        }:_(RawOrigin::Signed(manager), node)
        verify {
            assert!(!crate::registrar::ReservedList::<T>::contains_key(node));
        }
        register {
            // l is length of name.
            let l in 0..(LABEL_MAX_LEN as u32);
            let name = get_rand_name(l as usize);
            let rich_account = create_caller::<T,T::Currency>(8);
            let source = account_to_source::<T>(rich_account.clone());
        }:_(RawOrigin::Signed(rich_account), name.clone(),source,T::MinRegistrationDuration::get())
        verify {
            assert!(Pallet::<T>::check_expires_renewable(name_to_node::<T::Hash>(name,T::BaseNode::get()).into()).is_ok());
        }

        renew {
            // l is length of name.
            let l in 0..(LABEL_MAX_LEN as u32);
            let name = get_rand_name(l as usize);
            let rich_account = create_caller::<T,T::Currency>(8);
            let clone_rich = rich_account.clone();
            T::Currency::deposit_creating(&clone_rich,u32::MAX.into());
            Pallet::<T>::register(RawOrigin::Signed(clone_rich).into(), name.clone(),account_to_source::<T>(rich_account.clone()),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(rich_account),name,T::MinRegistrationDuration::get())


        set_owner {
            let name = get_rand_name(15);
            let hash = name_to_node::<T::Hash>(name.clone(),T::BaseNode::get()).into();
            let rich_account = create_caller::<T,T::Currency>(8);
            let clone_rich = rich_account.clone();
            let to_account = create_caller::<T,T::Currency>(2);
            Pallet::<T>::register(RawOrigin::Signed(clone_rich).into(), name,account_to_source::<T>(rich_account.clone()),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(rich_account),account_to_source::<T>(to_account.clone()),hash)


        mint_subname {
            let l in  0..(LABEL_MAX_LEN as u32);
            let name = get_rand_name(15);
            let hash = name_to_node::<T::Hash>(name.clone(),T::BaseNode::get()).into();
            let rich_account = create_caller::<T,T::Currency>(8);
            let clone_rich = rich_account.clone();
            Pallet::<T>::register(RawOrigin::Signed(clone_rich).into(), name,account_to_source::<T>(rich_account.clone()),T::MinRegistrationDuration::get())?;
            let subname = get_subname(l as usize);
            let subhash = get_subhash::<T>(&subname,hash);
            let clone_rich = rich_account.clone();
        }:_(RawOrigin::Signed(clone_rich),hash,subname,account_to_source::<T>(rich_account))


        reclaimed {
            let name = get_rand_name(15);
            let hash = name_to_node::<T::Hash>(name.clone(),T::BaseNode::get()).into();
            let rich_account = create_caller::<T,T::Currency>(8);
            let clone_rich = rich_account.clone();
            Pallet::<T>::register(RawOrigin::Signed(clone_rich).into(), name,account_to_source::<T>(rich_account.clone()),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(rich_account),hash)

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod redeem_code {
    use super::{get_manager, get_rand_name, name_to_node, poor_account};
    #[cfg(test)]
    use crate::mock::Test;
    use crate::redeem_code::crypto::{BenchCrypto, BenchPublic};
    use crate::traits::Registrar;
    use crate::{
        redeem_code::{Call, Config, Pallet},
        traits::{Label, LABEL_MAX_LEN},
    };
    use codec::Encode;
    use frame_benchmarking::benchmarks;
    use frame_system::RawOrigin;
    use sp_runtime::traits::IdentifyAccount;

    benchmarks! {
        where_clause {
            where
            T: crate::origin::Config + crate::registry::Config,
            BenchPublic: BenchCrypto<T::Public,T::Signature>,
            T::Public: Default + IdentifyAccount<AccountId = T::AccountId>,
        }
        mint_redeem {
            let l in 1..10_000;
        }:_(RawOrigin::Signed(get_manager::<T>()),0,l)

        name_redeem {
            let l in 1..LABEL_MAX_LEN as u32;
            let nouce = 5;
            let name = get_rand_name(l as usize);
            let (label, _) = Label::<T::Hash>::new(&name).unwrap();
            let label_node = label.node;
            let duration = <T as crate::redeem_code::pallet::Config>::Moment::from(24*60*60*365 as u32);
            let msg = (label_node, duration, nouce).encode();

            let public = T::Public::default();
            let signature = <BenchPublic as BenchCrypto<T::Public,T::Signature>>::sign(T::Public::default(),&msg);
            let official = public.into_account();

            crate::registry::Pallet::<T>::set_official(RawOrigin::Signed(get_manager::<T>()).into(),official)?;
            Pallet::<T>::mint_redeem(RawOrigin::Signed(get_manager::<T>()).into(),0,10)?;
            let hash = name_to_node::<T::Hash>(name.clone(),<T as Config>::Registrar::basenode());
            let poor_account7 = poor_account::<T>(7);
            let poor_account77 = poor_account::<T>(77);
        }:_(RawOrigin::Signed(poor_account7),name,duration,nouce,signature,poor_account77)

        name_redeem_any {
            let l in 1..LABEL_MAX_LEN as u32;
            let nouce = 5;
            let name = get_rand_name(l as usize);
            let duration = <T as crate::redeem_code::pallet::Config>::Moment::from(24*60*60*365 as u32);
            let msg = (duration, nouce).encode();

            let public = T::Public::default();
            let signature = <BenchPublic as BenchCrypto<T::Public,T::Signature>>::sign(T::Public::default(),&msg);
            let official = public.into_account();

            crate::registry::Pallet::<T>::set_official(RawOrigin::Signed(get_manager::<T>()).into(),official)?;
            Pallet::<T>::mint_redeem(RawOrigin::Signed(get_manager::<T>()).into(),0,10)?;
            let hash = name_to_node::<T::Hash>(name.clone(),<T as Config>::Registrar::basenode());
            let poor_account7 = poor_account::<T>(7);
            let poor_account77 = poor_account::<T>(77);
        }:_(RawOrigin::Signed(poor_account7),name,duration,nouce,signature,poor_account77)

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod price_oracle {
    use super::get_manager;
    #[cfg(test)]
    use crate::mock::Test;
    use crate::{
        price_oracle::{Call, Config, Pallet},
        traits::LABEL_MAX_LEN,
    };
    use frame_benchmarking::benchmarks;
    use frame_system::RawOrigin;

    benchmarks! {
        where_clause {
            where
            T: crate::origin::Config,
        }

        set_exchange_rate {
        }:_(RawOrigin::Signed(get_manager::<T>()),1000_u32.into())


        set_base_price {
            let l in 0..LABEL_MAX_LEN as u32;
        }:_(RawOrigin::Signed(get_manager::<T>()),sp_std::vec![996_u32.into();l as usize])

        set_rent_price {
            let l in 0..LABEL_MAX_LEN as u32;
        }:_(RawOrigin::Signed(get_manager::<T>()),sp_std::vec![996_u32.into();l as usize])

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod origin {
    use super::{account_to_source, get_manager, poor_account};
    #[cfg(test)]
    use crate::mock::Test;
    use crate::origin::{Call, Config, Pallet};
    use frame_benchmarking::benchmarks;
    use frame_system::RawOrigin;

    benchmarks! {
        where_clause {
            where
        }

        set_origin_true {
            let account = poor_account::<T>(7);
        }:set_origin(RawOrigin::Signed(get_manager::<T>()),account_to_source::<T>(account),true)

        set_origin_false {
            let account = poor_account::<T>(7);
            Pallet::<T>::set_origin(RawOrigin::Signed(get_manager::<T>()).into(),account_to_source::<T>(account.clone()),true)?;
        }:set_origin(RawOrigin::Signed(get_manager::<T>()),account_to_source::<T>(account),false)

        set_origin_for_root_true {
            let account = poor_account::<T>(7);
        }:set_origin_for_root(RawOrigin::Root,account_to_source::<T>(account),true)

        set_origin_for_root_false {
            let account = poor_account::<T>(7);
            Pallet::<T>::set_origin_for_root(RawOrigin::Root.into(),account_to_source::<T>(account.clone()),true)?;
        }:set_origin_for_root(RawOrigin::Root,account_to_source::<T>(account),false)

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}
