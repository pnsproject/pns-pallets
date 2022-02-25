use crate::*;
use codec::Encode;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use pns_resolvers::resolvers::{AddressKind, MultiAddress, TextKind};
use sp_runtime::testing::TestSignature;
use traits::Label;

const DAYS: u64 = 24 * 60 * 60;

#[test]
fn register_test() {
    new_test_ext().execute_with(|| {
        // now not supported chinese domain name
        let name = "中文测试".as_bytes();
        assert_noop!(
            Registrar::register(
                Origin::signed(RICH_ACCOUNT),
                name.to_vec(),
                RICH_ACCOUNT,
                MinRegistrationDuration::get()
            ),
            registrar::Error::<Test>::ParseLabelFailed
        );

        // lable lenth too short
        assert_noop!(
            Registrar::register(
                Origin::signed(RICH_ACCOUNT),
                b"hello".to_vec(),
                RICH_ACCOUNT,
                MinRegistrationDuration::get()
            ),
            registrar::Error::<Test>::LabelInvalid
        );

        let name = b"hello-world";

        let name2 = b"world-hello";

        // Minimal registration duration test
        assert_noop!(
            Registrar::register(
                Origin::signed(RICH_ACCOUNT),
                name.to_vec(),
                RICH_ACCOUNT,
                MinRegistrationDuration::get() - DAYS
            ),
            registrar::Error::<Test>::RegistryDurationInvalid
        );
        use traits::PriceOracle as _;

        let total_price =
            PriceOracle::registry_price(name.len(), MinRegistrationDuration::get()).unwrap();
        let init_free = Balances::free_balance(RICH_ACCOUNT);
        // a right call
        assert_ok!(Registrar::register(
            Origin::signed(RICH_ACCOUNT),
            name.to_vec(),
            RICH_ACCOUNT,
            MinRegistrationDuration::get()
        ));

        let now_free = Balances::free_balance(RICH_ACCOUNT);
        let deposit = PriceOracle::deposit_fee(name.len()).unwrap();
        let gas_fee = init_free - now_free - total_price - deposit;

        assert_eq!(gas_fee, 0);

        let (label, len) = Label::<Hash>::new(name).unwrap();

        let (label2, len2) = Label::<Hash>::new(name2).unwrap();

        assert!(len == 11);

        assert!(len2 == 11);
        let node = label.encode_with_node(&DOT_BASENODE);
        let node2 = label2.encode_with_node(&DOT_BASENODE);

        assert_ok!(Registry::approve(
            Origin::signed(RICH_ACCOUNT),
            9944,
            node,
            true
        ));

        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        let now = Timestamp::now();

        assert_eq!(info.expire, now + MinRegistrationDuration::get());

        assert_noop!(
            Registrar::register(
                Origin::signed(MONEY_ACCOUNT),
                name.to_vec(),
                MONEY_ACCOUNT,
                MinRegistrationDuration::get()
            ),
            registrar::Error::<Test>::Occupied
        );

        assert_noop!(
            Registrar::register(
                Origin::signed(POOR_ACCOUNT),
                name2.to_vec(),
                POOR_ACCOUNT,
                MinRegistrationDuration::get()
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );
        let price_free =
            PriceOracle::registry_price(name2.len(), MinRegistrationDuration::get()).unwrap();

        Balances::set_balance(Origin::root(), POOR_ACCOUNT, price_free, 0).unwrap();

        assert_noop!(
            Registrar::register(
                Origin::signed(POOR_ACCOUNT),
                name2.to_vec(),
                POOR_ACCOUNT,
                MinRegistrationDuration::get()
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );

        Balances::set_balance(Origin::root(), POOR_ACCOUNT, price_free * 2, 0).unwrap();

        assert_ok!(Registrar::register(
            Origin::signed(POOR_ACCOUNT),
            name2.to_vec(),
            POOR_ACCOUNT,
            MinRegistrationDuration::get()
        ));

        let renew_duration = 50 * DAYS;
        assert_ok!(Registrar::renew(
            Origin::signed(RICH_ACCOUNT),
            name.to_vec(),
            renew_duration
        ));

        let old_expire = info.expire;
        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        assert_noop!(
            Registrar::set_owner(Origin::signed(MONEY_ACCOUNT), RICH_ACCOUNT, node),
            registry::Error::<Test>::NoPermission
        );

        assert_eq!(info.expire, old_expire + renew_duration);

        assert_ok!(Registrar::renew(
            Origin::signed(MONEY_ACCOUNT),
            name.to_vec(),
            renew_duration
        ));

        let old_expire = info.expire;
        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        assert_eq!(info.expire, old_expire + renew_duration);

        assert!(Nft::is_owner(&RICH_ACCOUNT, (0, node)));

        assert_ok!(Registrar::set_owner(
            Origin::signed(RICH_ACCOUNT),
            MONEY_ACCOUNT,
            node
        ));

        assert!(Nft::is_owner(&MONEY_ACCOUNT, (0, node)));

        assert_ok!(Registrar::set_owner(
            Origin::signed(MONEY_ACCOUNT),
            RICH_ACCOUNT,
            node
        ));

        assert!(Nft::is_owner(&RICH_ACCOUNT, (0, node)));

        assert_ok!(Registrar::mint_subname(
            Origin::signed(RICH_ACCOUNT),
            node,
            b"test".to_vec(),
            MONEY_ACCOUNT
        ));

        assert_noop!(
            Registrar::mint_subname(
                Origin::signed(RICH_ACCOUNT),
                node,
                b"test".to_vec(),
                MONEY_ACCOUNT
            ),
            registrar::Error::<Test>::NotExistOrOccupied
        );

        assert_ok!(Registrar::mint_subname(
            Origin::signed(RICH_ACCOUNT),
            node,
            b"test1".to_vec(),
            MONEY_ACCOUNT
        ));

        assert!(Nft::is_owner(&POOR_ACCOUNT, (0, node2)));

        assert_ok!(Registrar::mint_subname(
            Origin::signed(POOR_ACCOUNT),
            node2,
            b"test1".to_vec(),
            MONEY_ACCOUNT
        ));

        assert_noop!(
            Registrar::mint_subname(
                Origin::signed(RICH_ACCOUNT),
                node2,
                b"test2".to_vec(),
                MONEY_ACCOUNT
            ),
            registry::Error::<Test>::NoPermission
        );

        let (test_label, _) = Label::<Hash>::new(b"test1").unwrap();
        let test_node = test_label.encode_with_node(&node2);

        assert!(Nft::is_owner(&MONEY_ACCOUNT, (0, test_node)));
    });
}

#[test]
fn redeem_code_test() {
    new_test_ext().execute_with(|| {
        assert_ok!(RedeemCode::mint_redeem(
            Origin::signed(MANAGER_ACCOUNT),
            0,
            10
        ));

        let nouce = 0_u32;
        let (label, _) = Label::<Hash>::new("cupnfish".as_bytes()).unwrap();
        let label_node = label.node;
        let duration = MinRegistrationDuration::get();

        let signature = (label_node, duration, nouce).encode();

        println!("{:?}", signature);

        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(RICH_ACCOUNT),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(1, vec![1, 2, 3, 4]),
                POOR_ACCOUNT
            ),
            redeem_code::Error::<Test>::InvalidSignature
        );

        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(RICH_ACCOUNT),
                b"cupnfishxxx".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(1, signature.clone()),
                POOR_ACCOUNT
            ),
            redeem_code::Error::<Test>::InvalidSignature
        );

        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(RICH_ACCOUNT),
                b"cupn---fish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(OFFICIAL_ACCOUNT, signature.clone()),
                POOR_ACCOUNT
            ),
            redeem_code::Error::<Test>::InvalidSignature
        );

        assert_ok!(RedeemCode::name_redeem(
            Origin::signed(RICH_ACCOUNT),
            b"cupnfish".to_vec(),
            MinRegistrationDuration::get(),
            0,
            TestSignature(OFFICIAL_ACCOUNT, signature.clone()),
            POOR_ACCOUNT
        ));

        let test_node = label.encode_with_node(&DOT_BASENODE);

        assert!(Nft::is_owner(&POOR_ACCOUNT, (0, test_node)));

        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(RICH_ACCOUNT),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(OFFICIAL_ACCOUNT, signature.clone()),
                POOR_ACCOUNT
            ),
            redeem_code::Error::<Test>::RedeemsHasBeenUsed
        );

        let nouce = 1_u32;
        let duration = MinRegistrationDuration::get();

        let signature = (duration, nouce).encode();

        assert_noop!(
            RedeemCode::name_redeem_any(
                Origin::signed(RICH_ACCOUNT),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(OFFICIAL_ACCOUNT, vec![1, 2, 3, 4]),
                POOR_ACCOUNT
            ),
            redeem_code::Error::<Test>::RedeemsHasBeenUsed
        );

        assert_noop!(
            RedeemCode::name_redeem_any(
                Origin::signed(RICH_ACCOUNT),
                b"cup-nfi--sh".to_vec(),
                MinRegistrationDuration::get(),
                1,
                TestSignature(OFFICIAL_ACCOUNT, signature.clone()),
                POOR_ACCOUNT
            ),
            redeem_code::Error::<Test>::ParseLabelFailed
        );

        assert_noop!(
            RedeemCode::name_redeem_any(
                Origin::signed(RICH_ACCOUNT),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                1,
                TestSignature(OFFICIAL_ACCOUNT, signature.clone()),
                POOR_ACCOUNT
            ),
            redeem_code::Error::<Test>::LabelLenInvalid
        );

        assert_ok!(Registrar::register(
            Origin::signed(RICH_ACCOUNT),
            b"cupnfishqqq".to_vec(),
            POOR_ACCOUNT,
            MinRegistrationDuration::get()
        ));

        assert_noop!(
            RedeemCode::name_redeem_any(
                Origin::signed(RICH_ACCOUNT),
                b"cupnfishqqq".to_vec(),
                MinRegistrationDuration::get(),
                1,
                TestSignature(OFFICIAL_ACCOUNT, signature.clone()),
                POOR_ACCOUNT
            ),
            registrar::Error::<Test>::Occupied
        );

        assert_ok!(RedeemCode::name_redeem_any(
            Origin::signed(RICH_ACCOUNT),
            b"cupnfishxxx".to_vec(),
            MinRegistrationDuration::get(),
            1,
            TestSignature(OFFICIAL_ACCOUNT, signature.clone()),
            POOR_ACCOUNT
        ));

        let test_node = Label::new("cupnfishxxx".as_bytes())
            .unwrap()
            .0
            .encode_with_node(&DOT_BASENODE);

        assert!(Nft::is_owner(&POOR_ACCOUNT, (0, test_node)));
    })
}

#[test]
fn resolvers_test() {
    new_test_ext().execute_with(|| {
        assert_ok!(Registrar::register(
            Origin::signed(RICH_ACCOUNT),
            b"cupnfishxxx".to_vec(),
            MONEY_ACCOUNT,
            MinRegistrationDuration::get()
        ));

        let node = Label::new("cupnfishxxx".as_bytes())
            .unwrap()
            .0
            .encode_with_node(&DOT_BASENODE);

        assert_ok!(Resolvers::set_account(
            Origin::signed(MONEY_ACCOUNT),
            node,
            AddressKind::Substrate,
            MultiAddress::Id(POOR_ACCOUNT)
        ));
        assert_ok!(Resolvers::set_account(
            Origin::signed(MONEY_ACCOUNT),
            node,
            AddressKind::Ethereum,
            MultiAddress::Address20([4; 20])
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Email,
            b"cupnfish@qq.com".to_vec().into(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Url,
            b"www.baidu.com".to_vec().into(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Avatar,
            b"cupnfish".to_vec().into(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Description,
            b"A Rust programer.".to_vec().into(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Notice,
            b"test notice".to_vec().into(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Keywords,
            b"test,key,words,show".to_vec().into(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Twitter,
            b"twitter address".to_vec().into(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Github,
            b"github homepage".to_vec().into(),
        ));
        assert_noop!(
            Resolvers::set_account(
                Origin::signed(RICH_ACCOUNT),
                node,
                AddressKind::Substrate,
                MultiAddress::Id(POOR_ACCOUNT)
            ),
            pns_resolvers::resolvers::Error::<Test>::InvalidPermission
        );
    })
}

#[test]
fn label_test() {
    // 中文 test
    assert!(Label::<Hash>::new("中文域名暂不支持".as_bytes()).is_none());

    // white space test
    assert!(Label::<Hash>::new("hello world".as_bytes()).is_none());

    // dot test
    assert!(Label::<Hash>::new("hello.world".as_bytes()).is_none());

    // '-' test
    assert!(Label::<Hash>::new("-hello".as_bytes()).is_none());
    assert!(Label::<Hash>::new("he-llo".as_bytes()).is_none());
    assert!(Label::<Hash>::new("he--llo".as_bytes()).is_none());
    assert!(Label::<Hash>::new("hello-".as_bytes()).is_none());

    // normal label test
    assert!(Label::<Hash>::new("hello".as_bytes()).is_some());
    assert!(Label::<Hash>::new("111hello".as_bytes()).is_some());
    assert!(Label::<Hash>::new("123455".as_bytes()).is_some());
    assert!(Label::<Hash>::new("0x1241513".as_bytes()).is_some());

    // result test
    assert_eq!(
        Label::<Hash>::new("dot".as_bytes())
            .unwrap()
            .0
            .to_basenode(),
        DOT_BASENODE
    )
}
