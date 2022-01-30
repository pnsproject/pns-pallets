//! Benchmarking setup for pns-pallets
#![cfg(feature = "runtime-benchmarks")]

use sp_core::H256;

pub fn get_rand_name(len: usize) -> Vec<u8> {
    let mut name = "cupnfishxx".to_string();
    for _ in 10..len {
        name.push_str("x");
    }
    name.into_bytes()
}

pub fn name_to_node(name: Vec<u8>) -> H256 {
    let (label, _len) = crate::traits::Label::<H256>::new(&name).unwrap();
    label.encode_with_basenode(crate::mock::BaseNode::get())
}

mod registry {
    use crate::mock::{AccountId, Hash, Test, MANAGER_ACCOUNT, OFFICIAL_ACCOUNT};
    use crate::{
        registry::{Call, Config, Pallet},
        traits::Label,
    };
    use frame_benchmarking::{account, benchmarks};
    use frame_system::RawOrigin;
    use sp_runtime::traits::StaticLookup;
    use sp_runtime::DispatchError;
    const SEED: u32 = 996;

    fn get_account_and_node<T: Config>(
        name: &'static str,
        index: u32,
    ) -> Result<(T::AccountId, T::Hash), DispatchError>
    where
        T: Config,
        T::Hash: From<Hash>,
        T::AccountId: From<AccountId> + Clone,
    {
        let owner = account::<T::AccountId>(name, index, SEED);
        let node = Label::<T::Hash>::new(format!("{name}{index}").as_bytes())
            .unwrap()
            .0
            .node;
        crate::registry::Pallet::<T>::_mint_subname(
            &T::AccountId::from(OFFICIAL_ACCOUNT),
            Default::default(),
            T::Hash::from(crate::tests::BASE_NODE),
            node,
            owner.clone(),
            20,
            |_| Ok(()),
        )?;

        Ok((owner, node))
    }

    benchmarks! {
        where_clause {
            where
            T::ResolverId: From<u32>,
            T::ClassId: From<u32>,
            T::Hash: From<Hash>,
            T::AccountId: From<AccountId> + Clone,
            T::Lookup: StaticLookup<Target = T::AccountId,Source = T::AccountId>,
        }
        approval_for_all_true {
            let caller = account::<T::AccountId>("caller",0,SEED);
            let operator = account::<T::AccountId>("operator",1,SEED);
            let approved = true;
        }: approval_for_all(RawOrigin::Signed(caller.clone()), operator.clone(),approved)
        verify {
            assert_eq!(crate::registry::OperatorApprovals::<T>::contains_key(caller,operator), approved);
        }
        approval_for_all_false {
            let caller = account::<T::AccountId>("caller",0,SEED);
            let operator = account::<T::AccountId>("operator",1,SEED);
            let approved = false;
            Pallet::<T>::approval_for_all(RawOrigin::Signed(caller.clone()).into(), operator.clone(),!approved)?;
        }: approval_for_all(RawOrigin::Signed(caller.clone()), operator.clone(),approved)
        verify {
            assert_eq!(crate::registry::OperatorApprovals::<T>::contains_key(caller,operator), approved);
        }
        set_resolver {
            let (owner,node) = get_account_and_node::<T>("caller",0)?;
        }: _(RawOrigin::Signed(owner), node,T::ResolverId::from(567))
        verify {
            assert_eq!(crate::registry::Resolver::<T>::get(node), T::ResolverId::from(567));
        }
        destroy {
            let (owner,node) = get_account_and_node::<T>("caller",3)?;
        }: _(RawOrigin::Signed(owner), node)
        verify {
            assert!(!crate::nft::Tokens::<T>::contains_key(T::ClassId::from(0),node));
        }
        set_official {
            let official = account::<T::AccountId>("official",567,SEED);
        }: _(RawOrigin::Signed(MANAGER_ACCOUNT.into()), official.clone())
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
    use super::{get_rand_name, name_to_node};
    use crate::mock::{
        AccountId, Hash, Moment, Test, MANAGER_ACCOUNT, OFFICIAL_ACCOUNT, POOR_ACCOUNT,
        RICH_ACCOUNT,
    };
    use crate::{
        registrar::{Call, Config, Pallet},
        traits::{Label, Registrar, LABEL_MAX_LEN, NFT},
    };
    use frame_benchmarking::benchmarks;
    use frame_support::traits::Get;
    use frame_system::RawOrigin;
    use sp_runtime::traits::{Saturating, StaticLookup};

    fn get_rand_node<T: Config>(seed: u32) -> T::Hash
    where
        T::Hash: From<Hash>,
    {
        sp_core::convert_hash::<Hash, [u8; 32]>(&sp_core::hashing::keccak_256(
            format!("rand{seed}").as_bytes(),
        ))
        .into()
    }

    fn get_subname(len: usize) -> Vec<u8> {
        let mut name = "abc".to_string();
        for _ in 10..len {
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
            T::AccountId: From<AccountId> + Clone,
            T::Hash: From<Hash>,
            T::Lookup: StaticLookup<Target = T::AccountId,Source = T::AccountId>,
            T::Moment: From<Moment>,
            T::Registry: NFT<T::AccountId,TokenId = T::Hash,ClassId = u32>,
        }
        add_reserved {
            let node = get_rand_node::<T>(567);
        }:_(RawOrigin::Signed(MANAGER_ACCOUNT.into()), node)
        verify {
            assert!(crate::registrar::ReservedList::<T>::contains_key(node));
        }
        remove_reserved {
            let node = get_rand_node::<T>(567);
            Pallet::<T>::add_reserved(RawOrigin::Signed(MANAGER_ACCOUNT.into()).into(), node)?;
        }:_(RawOrigin::Signed(MANAGER_ACCOUNT.into()), node)
        verify {
            assert!(!crate::registrar::ReservedList::<T>::contains_key(node));
        }
        register {
            // l is length of name.
            let l in 0..LABEL_MAX_LEN as u32;
            let name = get_rand_name(l as usize);
        }:_(RawOrigin::Signed(RICH_ACCOUNT.into()), name.clone(),RICH_ACCOUNT.into(),T::MinRegistrationDuration::get())
        verify {
            assert!(Pallet::<T>::check_expires_renewable(name_to_node(name).into()).is_ok());
        }

        renew {
            // l is length of name.
            let l in 0..LABEL_MAX_LEN as u32;
            let name = get_rand_name(l as usize);
            Pallet::<T>::register(RawOrigin::Signed(RICH_ACCOUNT.into()).into(), name.clone(),RICH_ACCOUNT.into(),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(RICH_ACCOUNT.into()),name.clone(),T::MinRegistrationDuration::get())
        verify {
            assert_eq!(crate::registrar::RegistrarInfos::<T>::get(T::Hash::from(name_to_node(name))).unwrap().expire,T::MinRegistrationDuration::get().saturating_mul(T::Moment::from(2)));
        }

        set_owner {
            let name = get_rand_name(15);
            let hash = name_to_node(name.clone()).into();
            Pallet::<T>::register(RawOrigin::Signed(RICH_ACCOUNT.into()).into(), name,RICH_ACCOUNT.into(),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(RICH_ACCOUNT.into()),POOR_ACCOUNT.into(),hash)
        verify {
            assert_eq!(T::Registry::owner((Default::default(),hash)),Some(POOR_ACCOUNT.into()));
        }

        mint_subname {
            let l in 0..LABEL_MAX_LEN as u32;
            let name = get_rand_name(15);
            let hash = name_to_node(name.clone()).into();
            Pallet::<T>::register(RawOrigin::Signed(RICH_ACCOUNT.into()).into(), name,RICH_ACCOUNT.into(),T::MinRegistrationDuration::get())?;
            let subname = get_subname(l as usize);
            let subhash = get_subhash::<T>(&subname,hash);
        }:_(RawOrigin::Signed(RICH_ACCOUNT.into()),hash,subname,POOR_ACCOUNT.into())
        verify {
            assert_eq!(T::Registry::owner((Default::default(),subhash)),Some(POOR_ACCOUNT.into()));
        }

        reclaimed {
            let name = get_rand_name(15);
            let hash = name_to_node(name.clone()).into();
            Pallet::<T>::register(RawOrigin::Signed(RICH_ACCOUNT.into()).into(), name,RICH_ACCOUNT.into(),T::MinRegistrationDuration::get())?;
        }:_(RawOrigin::Signed(RICH_ACCOUNT.into()),hash)
        verify {
            assert_eq!(T::Registry::owner((Default::default(),hash)),Some(OFFICIAL_ACCOUNT.into()));
        }
        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod redeem_code {
    use super::{get_rand_name, name_to_node};
    use crate::mock::{
        AccountId, Hash, MinRegistrationDuration, Moment, Test, MANAGER_ACCOUNT, OFFICIAL_ACCOUNT,
        POOR_ACCOUNT, RICH_ACCOUNT,
    };
    use crate::{
        redeem_code::{Call, Config, Pallet},
        traits::{Label, Registrar, LABEL_MAX_LEN},
    };
    use codec::Encode;
    use frame_benchmarking::benchmarks;
    use frame_system::RawOrigin;
    use sp_runtime::testing::TestSignature;

    benchmarks! {
        where_clause {
            where
            T::AccountId: From<AccountId> + Clone,
            T::Hash: From<Hash>,
            T::Signature: From<TestSignature>,
            T::Moment: From<Moment>,
        }
        mint_redeem {
            let l in 1..10_000;
        }:_(RawOrigin::Signed(MANAGER_ACCOUNT.into()),0,l)

        name_redeem {
            let l in 1..LABEL_MAX_LEN as u32;
            let nouce = l;
            let name = "cupnfish";
            let (label, _) = Label::<T::Hash>::new(name.as_bytes()).unwrap();
            let label_node = label.node;
            let duration = MinRegistrationDuration::get();
            let signature = (label_node, duration, nouce).encode();
            Pallet::<T>::mint_redeem(RawOrigin::Signed(MANAGER_ACCOUNT.into()).into(),0,l)?;
            let hash = name_to_node(name.as_bytes().to_vec());
        }:_(RawOrigin::Signed(RICH_ACCOUNT.into()),name.as_bytes().to_vec(),MinRegistrationDuration::get().into(),nouce,TestSignature(OFFICIAL_ACCOUNT, signature.clone()).into(),POOR_ACCOUNT.into())
        verify {
            assert!(T::Registrar::check_expires_renewable(hash.into()).is_ok());
        }

        name_redeem_any {
            let l in 1..LABEL_MAX_LEN as u32;
            let nouce = l;
            let name = get_rand_name(l as usize);
            let duration = MinRegistrationDuration::get();
            let signature = (duration, nouce).encode();
            Pallet::<T>::mint_redeem(RawOrigin::Signed(MANAGER_ACCOUNT.into()).into(),0,l)?;
            let hash = name_to_node(name.clone());
        }:_(RawOrigin::Signed(RICH_ACCOUNT.into()),name,MinRegistrationDuration::get().into(),nouce,TestSignature(OFFICIAL_ACCOUNT, signature.clone()).into(),POOR_ACCOUNT.into())
        verify {
            assert!(T::Registrar::check_expires_renewable(hash.into()).is_ok());
        }

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod price_oracle {
    use crate::mock::{AccountId, Balance, Test, MANAGER_ACCOUNT};
    use crate::{
        price_oracle::{Call, Config, Pallet},
        traits::LABEL_MAX_LEN,
    };
    use frame_benchmarking::benchmarks;
    use frame_support::traits::Currency;
    use frame_system::RawOrigin;

    benchmarks! {
        where_clause {
            where
            T::AccountId: From<AccountId> + Clone,
            T::Currency: Currency<T::AccountId,Balance = Balance>,
        }

        set_exchange_rate {
        }:_(RawOrigin::Signed(MANAGER_ACCOUNT.into()),1000)


        set_base_price {
            let l in 0..LABEL_MAX_LEN as u32;
        }:_(RawOrigin::Signed(MANAGER_ACCOUNT.into()),vec![996;l as usize])

        set_rent_price {
            let l in 0..LABEL_MAX_LEN as u32;
        }:_(RawOrigin::Signed(MANAGER_ACCOUNT.into()),vec![996;l as usize])

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod origin {
    use crate::mock::{AccountId, Test, MANAGER_ACCOUNT, POOR_ACCOUNT};
    use crate::origin::{Call, Config, Pallet};
    use frame_benchmarking::benchmarks;
    use frame_system::RawOrigin;
    use sp_runtime::traits::StaticLookup;
    benchmarks! {
        where_clause {
            where
            T::AccountId: From<AccountId> + Clone,
            T::Lookup: StaticLookup<Target = T::AccountId,Source = T::AccountId>,
        }

        set_origin_true {
        }:set_origin(RawOrigin::Signed(MANAGER_ACCOUNT.into()),POOR_ACCOUNT.into(),true)

        set_origin_false {
            Pallet::<T>::set_origin(RawOrigin::Signed(MANAGER_ACCOUNT.into()).into(),POOR_ACCOUNT.into(),true)?;
        }:set_origin(RawOrigin::Signed(MANAGER_ACCOUNT.into()),POOR_ACCOUNT.into(),false)

        set_origin_for_root_true {
        }:set_origin_for_root(RawOrigin::Root,POOR_ACCOUNT.into(),true)

        set_origin_for_root_false {
            Pallet::<T>::set_origin_for_root(RawOrigin::Root.into(),POOR_ACCOUNT.into(),true)?;
        }:set_origin_for_root(RawOrigin::Root,POOR_ACCOUNT.into(),false)

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
    }
}

mod resolvers {
    use crate::mock::{get_cupnfishu_node, AccountId, Hash, Test, MANAGER_ACCOUNT};
    use crate::resolvers::{AddressKind, Call, Config, Pallet, TextKind};
    use frame_benchmarking::benchmarks;
    use frame_system::RawOrigin;

    benchmarks! {
        where_clause {
            where
            T::AccountId: From<AccountId>,
            T::DomainHash: From<Hash>,
        }

        set_account {
        }: _(RawOrigin::Signed(MANAGER_ACCOUNT.into()),get_cupnfishu_node().into(),AddressKind::Substrate,Default::default())

        set_text {
            let l in 0..10_000;
        }: _(RawOrigin::Signed(MANAGER_ACCOUNT.into()), get_cupnfishu_node().into(),TextKind::Email,vec![9;l as usize])

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(),Test);
    }
}
