use crate::*;
use codec::Encode;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use pns_resolvers::resolvers::{Address, TextKind};
use sp_runtime::testing::TestSignature;
use traits::Label;

const DAYS: u64 = 24 * 60 * 60;
// 注册测试
#[test]
fn register_test() {
    new_test_ext().execute_with(|| {
        // now not supported chinese domain name
        // 测试中文域名是否可行
        let name = "中文测试".as_bytes();
        // 中文域名暂时不支持
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
        // 注册要求大于一定长度
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
        // 最小注册日期测试
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
        // 价格测试
        let total_price =
            PriceOracle::register_fee(name.len(), MinRegistrationDuration::get()).unwrap();
        let init_free = Balances::free_balance(RICH_ACCOUNT);
        // a right call
        // 正确注册name
        assert_ok!(Registrar::register(
            Origin::signed(RICH_ACCOUNT),
            name.to_vec(),
            RICH_ACCOUNT,
            MinRegistrationDuration::get()
        ));
        // 计算注册所用开销
        let now_free = Balances::free_balance(RICH_ACCOUNT);
        let deposit = PriceOracle::deposit_fee(name.len()).unwrap();
        let gas_fee = init_free - now_free - total_price - deposit;
        // 确保测试环境的收支平衡
        assert_eq!(gas_fee, 0);

        let (label, len) = Label::<Hash>::new(name).unwrap();

        let (label2, len2) = Label::<Hash>::new(name2).unwrap();
        // 确保label长度符合预期
        assert!(len == 11);

        assert!(len2 == 11);

        let node = label.encode_with_node(&DOT_BASENODE);
        let node2 = label2.encode_with_node(&DOT_BASENODE);
        // 授权node给9944
        assert_ok!(Registry::approve(
            Origin::signed(RICH_ACCOUNT),
            9944,
            node,
            true
        ));
        // 获取node的info
        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        let now = Timestamp::now();
        // 校验node注册的期限
        assert_eq!(info.expire, now + MinRegistrationDuration::get());
        // 测试被占用的域名是否可以被注册
        assert_noop!(
            Registrar::register(
                Origin::signed(MONEY_ACCOUNT),
                name.to_vec(),
                MONEY_ACCOUNT,
                MinRegistrationDuration::get()
            ),
            registrar::Error::<Test>::Occupied
        );
        // 测试没钱的账户是否可以注册
        assert_noop!(
            Registrar::register(
                Origin::signed(POOR_ACCOUNT),
                name2.to_vec(),
                POOR_ACCOUNT,
                MinRegistrationDuration::get()
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );
        // 给没钱的账户充钱
        let price_free =
            PriceOracle::register_fee(name2.len(), MinRegistrationDuration::get()).unwrap();

        Balances::set_balance(Origin::root(), POOR_ACCOUNT, price_free, 0).unwrap();
        // 测试充钱之后的账户仍然很穷（还有押金的金额，以及甚至生产环境下的gas费用）
        assert_noop!(
            Registrar::register(
                Origin::signed(POOR_ACCOUNT),
                name2.to_vec(),
                POOR_ACCOUNT,
                MinRegistrationDuration::get()
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );
        // 加钱
        Balances::set_balance(Origin::root(), POOR_ACCOUNT, price_free * 2, 0).unwrap();
        // 钱够了可以正确注册
        assert_ok!(Registrar::register(
            Origin::signed(POOR_ACCOUNT),
            name2.to_vec(),
            POOR_ACCOUNT,
            MinRegistrationDuration::get()
        ));
        // 续费测试
        let renew_duration = 50 * DAYS;
        assert_ok!(Registrar::renew(
            Origin::signed(RICH_ACCOUNT),
            name.to_vec(),
            renew_duration
        ));
        // 校验续费之后的时间是否正确
        let old_expire = info.expire;
        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        assert_noop!(
            Registrar::set_owner(Origin::signed(MONEY_ACCOUNT), RICH_ACCOUNT, node),
            registry::Error::<Test>::NoPermission
        );

        assert_eq!(info.expire, old_expire + renew_duration);
        // 给非自己的域名续费
        assert_ok!(Registrar::renew(
            Origin::signed(MONEY_ACCOUNT),
            name.to_vec(),
            renew_duration
        ));
        // 校验续费后的时间
        let old_expire = info.expire;
        let info = registrar::RegistrarInfos::<Test>::get(node).unwrap();

        assert_eq!(info.expire, old_expire + renew_duration);
        // 校验node的所有者
        assert!(Nft::is_owner(&RICH_ACCOUNT, (0, node)));
        // 测试域名交易
        assert_ok!(Registrar::set_owner(
            Origin::signed(RICH_ACCOUNT),
            MONEY_ACCOUNT,
            node
        ));
        // 校验交易是否成功
        assert!(Nft::is_owner(&MONEY_ACCOUNT, (0, node)));
        // 再次测试交易
        assert_ok!(Registrar::set_owner(
            Origin::signed(MONEY_ACCOUNT),
            RICH_ACCOUNT,
            node
        ));
        // 核验交易成功
        assert!(Nft::is_owner(&RICH_ACCOUNT, (0, node)));
        // 测试注册子域名
        assert_ok!(Registrar::mint_subname(
            Origin::signed(RICH_ACCOUNT),
            node,
            b"test".to_vec(),
            MONEY_ACCOUNT
        ));
        // 测试注册被占用的子域名
        assert_noop!(
            Registrar::mint_subname(
                Origin::signed(RICH_ACCOUNT),
                node,
                b"test".to_vec(),
                MONEY_ACCOUNT
            ),
            registrar::Error::<Test>::NotExistOrOccupied
        );
        // 给其他账户注册子域名测试
        assert_ok!(Registrar::mint_subname(
            Origin::signed(RICH_ACCOUNT),
            node,
            b"test1".to_vec(),
            MONEY_ACCOUNT
        ));
        // 校验注册后的所有者
        assert!(Nft::is_owner(&POOR_ACCOUNT, (0, node2)));
        // 自己的域名进行子域名注册测试
        assert_ok!(Registrar::mint_subname(
            Origin::signed(POOR_ACCOUNT),
            node2,
            b"test1".to_vec(),
            MONEY_ACCOUNT
        ));
        // 别人的域名进行子域名注册测试，权限应该不足
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
        // 校验域名的所有者
        assert!(Nft::is_owner(&MONEY_ACCOUNT, (0, test_node)));
    });
}
// 兑换码功能测试
#[test]
fn redeem_code_test() {
    new_test_ext().execute_with(|| {
        // 铸造0到10兑换码nouce
        assert_ok!(RedeemCode::mint_redeem(
            Origin::signed(MANAGER_ACCOUNT),
            0,
            10
        ));
        // 生成兑换码
        let nouce = 0_u32;
        let (label, _) = Label::<Hash>::new("cupnfish".as_bytes()).unwrap();
        let label_node = label.node;
        let duration = MinRegistrationDuration::get();

        let signature = (label_node, duration, nouce).encode();

        println!("{:?}", signature);
        // 使用兑换码，但是签名出错
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
        // 使用兑换码，但是域名出错
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
        // 使用兑换码，但是域名及膝出错
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
        // 正确使用兑换码
        assert_ok!(RedeemCode::name_redeem(
            Origin::signed(RICH_ACCOUNT),
            b"cupnfish".to_vec(),
            MinRegistrationDuration::get(),
            0,
            TestSignature(OFFICIAL_ACCOUNT, signature.clone()),
            POOR_ACCOUNT
        ));
        // 查看是否正确注册
        // 先计算出测试的域名哈希
        let test_node = label.encode_with_node(&DOT_BASENODE);
        // 使用该哈希查询该域名所有者
        assert!(Nft::is_owner(&POOR_ACCOUNT, (0, test_node)));
        // 测试使用过的签名能否再使用
        assert_noop!(
            RedeemCode::name_redeem(
                Origin::signed(RICH_ACCOUNT),
                b"cupnfish".to_vec(),
                MinRegistrationDuration::get(),
                0,
                TestSignature(OFFICIAL_ACCOUNT, signature),
                POOR_ACCOUNT
            ),
            redeem_code::Error::<Test>::RedeemsHasBeenUsed
        );
        // 制作另一个兑换码
        let nouce = 1_u32;
        let duration = MinRegistrationDuration::get();

        let signature = (duration, nouce).encode();
        // 使用过的nouce不能再使用
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
        // 不符合域名规则的域名不允许注册
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
        // 长度不符合要求的域名不准注册
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
        // 注册cupnfishqqq域名
        assert_ok!(Registrar::register(
            Origin::signed(RICH_ACCOUNT),
            b"cupnfishqqq".to_vec(),
            POOR_ACCOUNT,
            MinRegistrationDuration::get()
        ));
        // 被占用的域名不准注册
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
        // 未被占用的合理的域名可以注册
        assert_ok!(RedeemCode::name_redeem_any(
            Origin::signed(RICH_ACCOUNT),
            b"cupnfishxxx".to_vec(),
            MinRegistrationDuration::get(),
            1,
            TestSignature(OFFICIAL_ACCOUNT, signature),
            POOR_ACCOUNT
        ));
        // 校验是否注册成功
        let test_node = Label::new("cupnfishxxx".as_bytes())
            .unwrap()
            .0
            .encode_with_node(&DOT_BASENODE);

        assert!(Nft::is_owner(&POOR_ACCOUNT, (0, test_node)));
    })
}
// 解析器测试
#[test]
fn resolvers_test() {
    new_test_ext().execute_with(|| {
        // 注册一个 cupnfishxxx.dot 域名
        assert_ok!(Registrar::register(
            Origin::signed(RICH_ACCOUNT),
            b"cupnfishxxx".to_vec(),
            MONEY_ACCOUNT,
            MinRegistrationDuration::get()
        ));
        // 计算注册域名的哈希
        let node = Label::new("cupnfishxxx".as_bytes())
            .unwrap()
            .0
            .encode_with_node(&DOT_BASENODE);
        // 设置解析器账号
        assert_ok!(Resolvers::set_account(
            Origin::signed(MONEY_ACCOUNT),
            node,
            Address::Id(POOR_ACCOUNT),
        ));
        // 设置解析器账号，但格式为以太坊
        assert_ok!(Resolvers::set_account(
            Origin::signed(MONEY_ACCOUNT),
            node,
            Address::Ethereum([4; 20]),
        ));
        // 设置文本
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Email,
            b"cupnfish@qq.com".to_vec().into(),
        ));
        // 设置地址
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Url,
            b"www.baidu.com".to_vec().into(),
        ));
        // 设置头像
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Avatar,
            b"cupnfish".to_vec().into(),
        ));
        // 设置描述
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Description,
            b"A Rust programer.".to_vec().into(),
        ));
        // 设置注意
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Notice,
            b"test notice".to_vec().into(),
        ));
        // 设置关键字
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Keywords,
            b"test,key,words,show".to_vec().into(),
        ));
        // 设置推特
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Twitter,
            b"twitter address".to_vec().into(),
        ));
        // 设置github
        assert_ok!(Resolvers::set_text(
            Origin::signed(MONEY_ACCOUNT),
            node,
            TextKind::Github,
            b"github homepage".to_vec().into(),
        ));
        // 另一个账号进行设置，没有权限，所以返回错误
        assert_noop!(
            Resolvers::set_account(
                Origin::signed(RICH_ACCOUNT),
                node,
                Address::Id(POOR_ACCOUNT),
            ),
            pns_resolvers::resolvers::Error::<Test>::InvalidPermission
        );
    })
}

// 对域名解析测试
#[test]
fn label_test() {
    // 中文 test
    assert!(Label::<Hash>::new("中文域名暂不支持".as_bytes()).is_none());

    // 空格测试
    assert!(Label::<Hash>::new("hello world".as_bytes()).is_none());

    // 点测试
    assert!(Label::<Hash>::new("hello.world".as_bytes()).is_none());

    // 横线测试
    assert!(Label::<Hash>::new("-hello".as_bytes()).is_none());
    assert!(Label::<Hash>::new("he-llo".as_bytes()).is_none());
    assert!(Label::<Hash>::new("he--llo".as_bytes()).is_none());
    assert!(Label::<Hash>::new("hello-".as_bytes()).is_none());

    // 普通的字符测试
    assert!(Label::<Hash>::new("hello".as_bytes()).is_some());
    assert!(Label::<Hash>::new("111hello".as_bytes()).is_some());
    assert!(Label::<Hash>::new("123455".as_bytes()).is_some());
    assert!(Label::<Hash>::new("0x1241513".as_bytes()).is_some());

    // 结果测试
    assert_eq!(
        Label::<Hash>::new("dot".as_bytes())
            .unwrap()
            .0
            .to_basenode(),
        DOT_BASENODE
    )
}
