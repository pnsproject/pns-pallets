use crate::*;
use codec::Encode;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use pns_resolvers::{AddressKind, TextKind};
use sp_runtime::testing::TestSignature;
use sp_runtime::MultiAddress;
use traits::Label;

const BASE: u128 = 1_000_000_000;
const DAYS: u64 = 24 * 60 * 60;

const BASE_NODE: Hash = sp_core::H256([
    206, 21, 156, 243, 67, 128, 117, 125, 25, 50, 168, 228, 167, 78, 133, 232, 89, 87, 176, 167,
    165, 45, 156, 86, 108, 10, 60, 141, 97, 51, 208, 247,
]);

#[test]
fn register_test() {
    new_test_ext().execute_with(|| {
        // init works
        let test_account = 10_086;
        let to_account = 10_000;
        let poor_account = 1_008_611;
        let official_account = 1_200;
        let manager = 111_111;

        initial(test_account, official_account, to_account, manager);

        // now not supported chinese domain name

        let name = "中文测试".as_bytes();
        assert_noop!(
            Registrar::register(
                Origin::signed(test_account),
                name.to_vec(),
                test_account,
                MinRegistrationDuration::get()
            ),
            registrar::Error::<Test>::ParseLabelFailed
        );

        // lable lenth too short
        assert_noop!(
            Registrar::register(
                Origin::signed(test_account),
                b"hello".to_vec(),
                test_account,
                MinRegistrationDuration::get()
            ),
            registrar::Error::<Test>::LabelInvalid
        );

        let name = b"hello-world";

        let name2 = b"world-hello";

        // Minimal registration duration test
        assert_noop!(
            Registrar::register(
                Origin::signed(test_account),
                name.to_vec(),
                test_account,
                MinRegistrationDuration::get() - DAYS
            ),
            registrar::Error::<Test>::RegistryDurationInvalid
        );
        use traits::PriceOracle as _;

        let total_price =
            PriceOracle::registry_price(name.len(), MinRegistrationDuration::get()).unwrap();
        let init_free = Balances::free_balance(test_account);
        let init_deposit = Balances::reserved_balance(official_account);
        // a right call
        assert_ok!(Registrar::register(
            Origin::signed(test_account),
            name.to_vec(),
            test_account,
            MinRegistrationDuration::get()
        ));

        let now_free = Balances::free_balance(test_account);
        let now_deposit = Balances::reserved_balance(official_account);

        let gas_fee = init_free - now_free - total_price;

        println!("gas fee: {}", gas_fee / BASE);

        assert_eq!(
            now_deposit - init_deposit,
            PriceOracle::deposit_fee(name.len()).unwrap()
        );

        let (label, len) = Label::<Hash>::new(name).unwrap();

        let (label2, len2) = Label::<Hash>::new(name2).unwrap();

        assert!(len == 11);

        assert!(len2 == 11);
        let node = label.encode_with_basenode(BASE_NODE);
        let node2 = label2.encode_with_basenode(BASE_NODE);
        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        let now = Timestamp::now();

        assert_eq!(info.expire, now + MinRegistrationDuration::get());

        assert_noop!(
            Registrar::register(
                Origin::signed(to_account),
                name.to_vec(),
                to_account,
                MinRegistrationDuration::get()
            ),
            registrar::Error::<Test>::Occupied
        );

        assert_noop!(
            Registrar::register(
                Origin::signed(poor_account),
                name2.to_vec(),
                poor_account,
                MinRegistrationDuration::get()
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );
        let price_free =
            PriceOracle::registry_price(name2.len(), MinRegistrationDuration::get()).unwrap();

        Balances::set_balance(Origin::root(), poor_account, price_free, 0).unwrap();

        assert_noop!(
            Registrar::register(
                Origin::signed(poor_account),
                name2.to_vec(),
                poor_account,
                MinRegistrationDuration::get()
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );

        Balances::set_balance(Origin::root(), poor_account, price_free * 2, 0).unwrap();

        assert_ok!(Registrar::register(
            Origin::signed(poor_account),
            name2.to_vec(),
            poor_account,
            MinRegistrationDuration::get()
        ));

        let renew_duration = 50 * DAYS;
        assert_ok!(Registrar::renew(
            Origin::signed(test_account),
            name.to_vec(),
            renew_duration
        ));

        let old_expire = info.expire;
        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        assert_noop!(
            Registrar::set_owner(Origin::signed(to_account), test_account, node),
            registry::Error::<Test>::NoPermission
        );

        assert_eq!(info.expire, old_expire + renew_duration);

        assert_ok!(Registrar::renew(
            Origin::signed(to_account),
            name.to_vec(),
            renew_duration
        ));

        let old_expire = info.expire;
        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        assert_eq!(info.expire, old_expire + renew_duration);

        assert!(Nft::is_owner(&test_account, (0, node)));

        assert_ok!(Registrar::set_owner(
            Origin::signed(test_account),
            to_account,
            node
        ));

        assert!(Nft::is_owner(&to_account, (0, node)));

        assert_ok!(Registrar::set_owner(
            Origin::signed(to_account),
            test_account,
            node
        ));

        assert!(Nft::is_owner(&test_account, (0, node)));

        assert_ok!(Registrar::mint_subname(
            Origin::signed(test_account),
            node,
            b"test".to_vec(),
            to_account
        ));

        assert_noop!(
            Registrar::mint_subname(
                Origin::signed(test_account),
                node,
                b"test".to_vec(),
                to_account
            ),
            registrar::Error::<Test>::NotExistOrOccupied
        );

        assert_ok!(Registrar::mint_subname(
            Origin::signed(test_account),
            node,
            b"test1".to_vec(),
            to_account
        ));

        assert!(Nft::is_owner(&poor_account, (0, node2)));

        assert_ok!(Registrar::mint_subname(
            Origin::signed(poor_account),
            node2,
            b"test1".to_vec(),
            to_account
        ));

        assert_noop!(
            Registrar::mint_subname(
                Origin::signed(test_account),
                node2,
                b"test2".to_vec(),
                to_account
            ),
            registry::Error::<Test>::NoPermission
        );

        let (test_label, _) = Label::<Hash>::new(b"test1").unwrap();
        let test_node = test_label.encode_with_node(node2);

        assert!(Nft::is_owner(&to_account, (0, test_node)));
    });
}

#[test]
fn redeem_code_test() {
    new_test_ext().execute_with(|| {
        // init works
        let test_account = 10_086;
        let to_account = 10_000;
        let poor_account = 1_008_611;
        let official_account = 1_200;
        let manager = 111_111;
        initial(test_account, official_account, to_account, manager);

        assert_ok!(RedeemCode::mint_redeem(Origin::signed(manager), 0, 10));

        let nouce = 0_u32;
        let (label, _) = Label::<Hash>::new("cupnfish".as_bytes()).unwrap();
        let label_node = label.node;
        let duration = MinRegistrationDuration::get();

        let signature = (label_node, duration, nouce).encode();

        println!("{:?}", signature);

        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(test_account),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(1, vec![1, 2, 3, 4]),
                poor_account
            ),
            redeem_code::Error::<Test>::InvalidSignature
        );

        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(test_account),
                b"cupnfishxxx".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(1, signature.clone()),
                poor_account
            ),
            redeem_code::Error::<Test>::InvalidSignature
        );

        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(test_account),
                b"cupn---fish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(official_account, signature.clone()),
                poor_account
            ),
            redeem_code::Error::<Test>::ParseLabelFailed
        );

        assert_ok!(RedeemCode::name_redeem(
            Origin::signed(test_account),
            b"cupnfish".to_vec(),
            MinRegistrationDuration::get(),
            0,
            TestSignature(official_account, signature.clone()),
            poor_account
        ));

        let test_node = label.encode_with_basenode(BASE_NODE);

        assert!(Nft::is_owner(&poor_account, (0, test_node)));

        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(test_account),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(official_account, signature.clone()),
                poor_account
            ),
            redeem_code::Error::<Test>::RedeemsHasBeenUsed
        );

        let nouce = 1_u32;
        let duration = MinRegistrationDuration::get();

        let signature = (duration, nouce).encode();

        assert_noop!(
            RedeemCode::name_redeem_any(
                Origin::signed(test_account),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(official_account, vec![1, 2, 3, 4]),
                poor_account
            ),
            redeem_code::Error::<Test>::RedeemsHasBeenUsed
        );

        assert_noop!(
            RedeemCode::name_redeem_any(
                Origin::signed(test_account),
                b"cupnfi--sh".to_vec(),
                MinRegistrationDuration::get(),
                1,
                TestSignature(official_account, signature.clone()),
                poor_account
            ),
            redeem_code::Error::<Test>::ParseLabelFailed
        );

        assert_noop!(
            RedeemCode::name_redeem_any(
                Origin::signed(test_account),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                1,
                TestSignature(official_account, signature.clone()),
                poor_account
            ),
            redeem_code::Error::<Test>::LabelLenInvalid
        );

        assert_ok!(Registrar::register(
            Origin::signed(test_account),
            b"cupnfishqqq".to_vec(),
            poor_account,
            MinRegistrationDuration::get()
        ));

        assert_noop!(
            RedeemCode::name_redeem_any(
                Origin::signed(test_account),
                b"cupnfishqqq".to_vec(),
                MinRegistrationDuration::get(),
                1,
                TestSignature(official_account, signature.clone()),
                poor_account
            ),
            registrar::Error::<Test>::Occupied
        );

        assert_ok!(RedeemCode::name_redeem_any(
            Origin::signed(test_account),
            b"cupnfishxxx".to_vec(),
            MinRegistrationDuration::get(),
            1,
            TestSignature(official_account, signature.clone()),
            poor_account
        ));

        let test_node = Label::new("cupnfishxxx".as_bytes())
            .unwrap()
            .0
            .encode_with_basenode(BASE_NODE);

        assert!(Nft::is_owner(&poor_account, (0, test_node)));
    })
}

#[test]
fn resolvers_test() {
    new_test_ext().execute_with(|| {
        // init works
        let test_account = 10_086;
        let to_account = 10_000;
        let poor_account = 1_008_611;
        let official_account = 1_200;
        let manager = 111_111;
        initial(test_account, official_account, to_account, manager);

        assert_ok!(Registrar::register(
            Origin::signed(test_account),
            b"cupnfishxxx".to_vec(),
            to_account,
            MinRegistrationDuration::get()
        ));

        let node = Label::new("cupnfishxxx".as_bytes())
            .unwrap()
            .0
            .encode_with_basenode(BASE_NODE);

        assert_ok!(Resolvers::set_account(
            Origin::signed(to_account),
            node,
            AddressKind::Substrate,
            MultiAddress::Id(poor_account)
        ));
        assert_ok!(Resolvers::set_account(
            Origin::signed(to_account),
            node,
            AddressKind::Ethereum,
            MultiAddress::Address20([4; 20])
        ));
        assert_ok!(Resolvers::set_account(
            Origin::signed(to_account),
            node,
            AddressKind::Bitcoin,
            MultiAddress::Raw([7; 23].to_vec())
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(to_account),
            node,
            TextKind::Email,
            b"cupnfish@qq.com".to_vec(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(to_account),
            node,
            TextKind::Url,
            b"www.baidu.com".to_vec(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(to_account),
            node,
            TextKind::Avatar,
            b"cupnfish".to_vec(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(to_account),
            node,
            TextKind::Description,
            b"A Rust programer.".to_vec(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(to_account),
            node,
            TextKind::Notice,
            b"test notice".to_vec(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(to_account),
            node,
            TextKind::Keywords,
            b"test,key,words,show".to_vec(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(to_account),
            node,
            TextKind::Twitter,
            b"twitter address".to_vec(),
        ));
        assert_ok!(Resolvers::set_text(
            Origin::signed(to_account),
            node,
            TextKind::Github,
            b"github homepage".to_vec(),
        ));
        assert_noop!(
            Resolvers::set_account(
                Origin::signed(test_account),
                node,
                AddressKind::Substrate,
                MultiAddress::Id(poor_account)
            ),
            pns_resolvers::Error::<Test>::InvalidPermission
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
    assert!(Label::<Hash>::new("he-llo".as_bytes()).is_some());
    assert!(Label::<Hash>::new("he--llo".as_bytes()).is_none());
    assert!(Label::<Hash>::new("hello-".as_bytes()).is_none());

    // normal label test
    assert!(Label::<Hash>::new("hello".as_bytes()).is_some());
    assert!(Label::<Hash>::new("111hello".as_bytes()).is_some());
    assert!(Label::<Hash>::new("123455".as_bytes()).is_some());
    assert!(Label::<Hash>::new("0x1241513".as_bytes()).is_some());

    // result test
    assert_eq!(
        Label::<Hash>::new("dot".as_bytes()).unwrap().0.node,
        BASE_NODE
    )
}

fn initial(
    init_account: AccountId,
    official: AccountId,
    to_account: AccountId,
    manager: AccountId,
) {
    let free = 9_000_000 * BASE;
    // 71855736000000
    // 9000000000000000
    println!("init balance: {}", free / BASE);
    Balances::set_balance(Origin::root(), init_account, free, 0).unwrap();
    Balances::set_balance(Origin::root(), to_account, free, 0).unwrap();

    let rent_prices = vec![9, 9, 8, 7, 6, 5, 4, 3, 2, 1, 1];
    let base_prices = vec![1_200, 1_000, 900, 800, 700, 600, 500, 400, 300, 200, 100]
        .into_iter()
        .map(|price| price * 100)
        .collect();
    Registry::set_official(Origin::root(), official).unwrap();
    Registry::add_manger(Origin::signed(official), manager).unwrap();

    PriceOracle::set_rent_price(Origin::signed(manager), rent_prices).unwrap();
    PriceOracle::set_base_price(Origin::signed(manager), base_prices).unwrap();
    Nft::create_class(&official, vec![2, 22, 222], ()).unwrap();
    Nft::mint(&official, (0, BASE_NODE), vec![1, 2, 3], Default::default()).unwrap();
}
