//! Benchmarking setup for pns-pallets
#![cfg(feature = "runtime-benchmarks")]

use crate::traits::{LABEL_MAX_LEN, LABEL_MIN_LEN, MIN_REGISTRABLE_LEN};
use frame_benchmarking::account;
use sp_runtime::traits::StaticLookup;
use sp_std::vec::Vec;

pub const SEED: u32 = 996;
pub const U32_LABEL_MAX_LEN: u32 = LABEL_MAX_LEN as u32;
pub const U32_LABEL_MIN_LEN: u32 = LABEL_MIN_LEN as u32;
pub const U32_MIN_REGISTRABLE_LEN: u32 = MIN_REGISTRABLE_LEN as u32;

fn get_name(len: usize) -> sp_std::vec::Vec<u8> {
    let mut res = alloc::string::String::with_capacity(len);
    (0..len).for_each(|_| {
        res.push('x');
    });
    res.into_bytes()
}

pub fn name_to_node<H>(name: Vec<u8>, basenode: H) -> H
where
    H: Default + AsMut<[u8]> + codec::Encode + Clone,
{
    let (label, _len) = crate::traits::Label::<H>::new(&name).unwrap();
    label.encode_with_node(basenode)
}

pub fn account_to_source<T: frame_system::Config>(
    account: T::AccountId,
) -> <T::Lookup as StaticLookup>::Source {
    <T::Lookup as StaticLookup>::unlookup(account)
}

pub fn get_manager<T: crate::origin::Config>() -> T::AccountId {
    crate::origin::Origins::<T>::iter_keys().next().unwrap()
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
        let node = label.encode_with_node(T::Registrar::basenode());

        crate::nft::Pallet::<T>::mint(
            &owner,
            (class_id, node),
            Default::default(),
            Default::default(),
        )?;
        use crate::registry::DomainTracing;
        use crate::registry::Origin;
        if Origin::<T>::get(T::Registrar::basenode()).is_some() {
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
            assert_eq!(crate::registry::Official::<T>::get(), Some(official));
        }
        approve_true {
            let (owner,node) = get_account_and_node::<T>("owner",567)?;
            let to = account::<T::AccountId>("to",996,SEED);
        }: approve(RawOrigin::Signed(owner), to.clone(),node,true)
        verify {
            assert!(crate::registry::TokenApprovals::<T>::contains_key(node,to));
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
    use super::{
        account_to_source, get_manager, get_name, name_to_node, SEED, U32_LABEL_MAX_LEN,
        U32_LABEL_MIN_LEN, U32_MIN_REGISTRABLE_LEN,
    };
    #[cfg(test)]
    use crate::mock::Test;
    use crate::{
        registrar::{Call, Config, Pallet},
        traits::{Label, Registrar, MIN_REGISTRABLE_LEN},
    };
    use frame_benchmarking::{account, benchmarks};
    use frame_support::traits::{Currency, Get};
    use frame_system::RawOrigin;
    use sp_runtime::SaturatedConversion;

    pub fn create_caller<T>(idx: u32) -> T::AccountId
    where
        T: frame_system::Config + pallet_balances::Config,
    {
        let caller: T::AccountId = account("caller", idx, SEED);
        pallet_balances::Pallet::<T>::set_balance(
            RawOrigin::Root.into(),
            account_to_source::<T>(caller.clone()),
            999_999_999_999_999u64.saturated_into(),
            Default::default(),
        )
        .unwrap();
        caller
    }

    fn get_rand_node<T: Config>(seed: u32) -> T::Hash {
        let name = alloc::format!("rand{seed}");
        let label = Label::<T::Hash>::new(name.as_bytes()).unwrap().0;
        label.node
    }

    fn get_subhash<T: Config>(subname: &[u8], node: T::Hash) -> T::Hash {
        let (label, _len) = Label::new(subname).unwrap();
        label.encode_with_node(node)
    }

    benchmarks! {
        where_clause {
            where
            T: crate::origin::Config + pallet_balances::Config,
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
            let l in U32_MIN_REGISTRABLE_LEN..U32_LABEL_MAX_LEN;
            let name = get_name(l as usize);
            let rich_account = create_caller::<T>(8);
            let source = account_to_source::<T>(rich_account.clone());
        }:_(RawOrigin::Signed(rich_account), name.clone(),source,T::MinRegistrationDuration::get())
        verify {
            assert!(Pallet::<T>::check_expires_renewable(name_to_node::<T::Hash>(name,T::BaseNode::get())).is_ok());
        }

        renew {
            // l is length of name.
            let l in U32_MIN_REGISTRABLE_LEN..U32_LABEL_MAX_LEN;
            let name = get_name(l as usize);
            let rich_account = create_caller::<T>(8);
            let clone_rich = rich_account.clone();
            T::Currency::deposit_creating(&clone_rich,u32::MAX.into());
            Pallet::<T>::register(RawOrigin::Signed(clone_rich).into(), name.clone(),account_to_source::<T>(rich_account.clone()),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(rich_account),name,T::MinRegistrationDuration::get())


        set_owner {
            let name = get_name(MIN_REGISTRABLE_LEN);
            let hash = name_to_node::<T::Hash>(name.clone(),T::BaseNode::get());
            let rich_account = create_caller::<T>(8);
            let clone_rich = rich_account.clone();
            let to_account = create_caller::<T>(2);
            Pallet::<T>::register(RawOrigin::Signed(clone_rich).into(), name,account_to_source::<T>(rich_account.clone()),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(rich_account),account_to_source::<T>(to_account),hash)


        mint_subname {
            let l in  U32_LABEL_MIN_LEN..U32_LABEL_MAX_LEN;
            let name = get_name(MIN_REGISTRABLE_LEN);
            let hash = name_to_node::<T::Hash>(name.clone(),T::BaseNode::get());
            let rich_account = create_caller::<T>(8);
            let clone_rich = rich_account.clone();
            Pallet::<T>::register(RawOrigin::Signed(clone_rich).into(), name,account_to_source::<T>(rich_account.clone()),T::MinRegistrationDuration::get())?;
            let subname = get_name(l as usize);
            let subhash = get_subhash::<T>(&subname,hash);
            let clone_rich = rich_account.clone();
        }:_(RawOrigin::Signed(clone_rich),hash,subname,account_to_source::<T>(rich_account))


        reclaimed {
            let name = get_name(MIN_REGISTRABLE_LEN);
            let hash = name_to_node::<T::Hash>(name.clone(),T::BaseNode::get());
            let rich_account = create_caller::<T>(8);
            let clone_rich = rich_account.clone();
            Pallet::<T>::register(RawOrigin::Signed(clone_rich).into(), name,account_to_source::<T>(rich_account.clone()),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(rich_account),hash)

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod redeem_code {
    use super::{get_manager, name_to_node, poor_account, U32_LABEL_MAX_LEN, U32_LABEL_MIN_LEN};
    use crate::traits::Registrar;
    use crate::{
        redeem_code::{Call, Config, Pallet},
        traits::Label,
    };
    use codec::Decode;
    use frame_benchmarking::benchmarks;
    use frame_system::RawOrigin;

    benchmarks! {
        where_clause {
            where
            T: crate::origin::Config + crate::registry::Config,
            T::Signature: Decode,
            T::AccountId: Decode,
        }

        name_redeem_min {
            let name = sp_std::vec![104, 120, 120];
            let duration = T::Moment::from(31536000_u32);
            let nouce = 5;
            let signature = T::Signature::decode(&mut &sp_std::vec![0, 229, 199, 81, 157, 241, 4, 157, 210, 38, 135, 222, 235, 38, 34, 192, 103, 30, 22, 80, 103, 169, 1, 150, 27, 177, 180, 162, 166, 18, 199, 178, 147, 115, 83, 174, 148, 221, 52, 101, 44, 22, 46, 84, 126, 48, 154, 45, 106, 125, 139, 217, 17, 59, 243, 210, 11, 77, 46, 200, 216, 98, 238, 110, 8][..]).unwrap();
            let official = T::AccountId::decode(&mut &sp_std::vec![13, 213, 60, 222, 83, 155, 9, 162, 203, 198, 116, 100, 154, 230, 209, 84, 224, 76, 72, 25, 6, 39, 161, 214, 157, 32, 78, 221, 137, 199, 207, 162][..]).unwrap();

            crate::registry::Pallet::<T>::set_official(RawOrigin::Signed(get_manager::<T>()).into(),official)?;
            Pallet::<T>::mint_redeem(RawOrigin::Signed(get_manager::<T>()).into(),0,10)?;
            let hash = name_to_node::<T::Hash>(name.clone(),<T as Config>::Registrar::basenode());
            let poor_account7 = poor_account::<T>(7);
            let poor_account77 = poor_account::<T>(77);
        }:name_redeem(RawOrigin::Signed(poor_account7),name,duration,nouce,signature,poor_account77)

        name_redeem_any_min {
            let name = sp_std::vec![99, 117, 112, 110, 102, 105, 115, 104, 120, 120];
            let duration = T::Moment::from(31536000_u32);
            let nouce = 5;
            let signature = T::Signature::decode(&mut &sp_std::vec![0, 182, 166, 0, 120, 22, 9, 41, 218, 6, 241, 55, 33, 5, 184, 6, 196, 87, 25, 50, 80, 73, 5, 245, 146, 120, 185, 202, 248, 52, 213, 24, 175, 10, 58, 41, 114, 237, 190, 72, 138, 70, 221, 151, 104, 249, 219, 191, 135, 243, 221, 29, 240, 231, 197, 177, 246, 248, 213, 114, 169, 60, 99, 167, 2][..]).unwrap();
            let official = T::AccountId::decode(&mut &sp_std::vec![13, 213, 60, 222, 83, 155, 9, 162, 203, 198, 116, 100, 154, 230, 209, 84, 224, 76, 72, 25, 6, 39, 161, 214, 157, 32, 78, 221, 137, 199, 207, 162][..]).unwrap();

            crate::registry::Pallet::<T>::set_official(RawOrigin::Signed(get_manager::<T>()).into(),official)?;
            Pallet::<T>::mint_redeem(RawOrigin::Signed(get_manager::<T>()).into(),0,10)?;
            let hash = name_to_node::<T::Hash>(name.clone(),<T as Config>::Registrar::basenode());
            let poor_account7 = poor_account::<T>(7);
            let poor_account77 = poor_account::<T>(77);
        }:name_redeem_any(RawOrigin::Signed(poor_account7),name,duration,nouce,signature,poor_account77)

        create_label {
            let l in U32_LABEL_MIN_LEN..U32_LABEL_MAX_LEN;
            let mut name = "hxx".to_ascii_lowercase();
            for _ in U32_LABEL_MIN_LEN..l {
                name.push('x');
            }
            let data = name.into_bytes();
        }: {
            crate::traits::Label::<T::Hash>::new(&data).unwrap();
        }

        for_redeem_code {
            let l in U32_LABEL_MIN_LEN..U32_LABEL_MAX_LEN;
            let mut name = "hxx".to_ascii_lowercase();
            for _ in U32_LABEL_MIN_LEN..l {
                name.push('x');
            }
            let data = name.into_bytes();
            let (label, _) =
            Label::<T::Hash>::new(&data).unwrap();
            let duration = <T as crate::redeem_code::pallet::Config>::Moment::from(24*60*60*365_u32);
            let poor_account7 = poor_account::<T>(7);
        }: {
            <T as Config>::Registrar::for_redeem_code(data, poor_account7, duration, label).unwrap();
        }

        mint_redeem {
            let l in 1..10_000;
        }:_(RawOrigin::Signed(get_manager::<T>()),0,l)

    }
}

mod price_oracle {
    use super::get_manager;
    #[cfg(test)]
    use crate::mock::Test;
    use crate::price_oracle::{Call, Config, Pallet};
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
        }:_(RawOrigin::Signed(get_manager::<T>()),[996_u32.into();11])

        set_rent_price {
        }:_(RawOrigin::Signed(get_manager::<T>()),[996_u32.into();11])

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

        set_registrar_open {
        }:_(RawOrigin::Signed(get_manager::<T>()),false)

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}
