use frame_support::parameter_types;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
pub type Block = frame_system::mocking::MockBlock<Test>;
pub type Hash = H256;
pub type Balance = u128;
pub type BlockNumber = u32;
pub type AccountId = u64;
pub const MILLISECS_PER_BLOCK: u64 = 6000;

pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// dot : [63, 206, 125, 19, 100, 168, 147, 226, 19, 188, 66, 18, 121, 43, 81, 127, 252, 136, 245, 177, 59, 134, 200, 239, 156, 141, 57, 12, 58, 19, 112, 206]
// ksm : [40, 176, 66, 80, 226, 106, 137, 121, 141, 170, 194, 128, 195, 181, 31, 184, 186, 190, 216, 60, 185, 180, 141, 134, 171, 252, 4, 74, 2, 250, 3, 144]
pub const DOT_BASENODE: H256 = sp_core::H256([
    63, 206, 125, 19, 100, 168, 147, 226, 19, 188, 66, 18, 121, 43, 81, 127, 252, 136, 245, 177,
    59, 134, 200, 239, 156, 141, 57, 12, 58, 19, 112, 206,
]);

// Configure a mock Test to test the pallet.
frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system,
        PriceOracle: crate::price_oracle,
        RedeemCode: crate::redeem_code,
        Registrar: crate::registrar,
        Registry: crate::registry,
        ManagerOrigin: crate::origin,
        Resolvers: pns_resolvers::resolvers,
        Nft: crate::nft,
        Balances: pallet_balances,
        Timestamp: pallet_timestamp,
        Aura: pallet_aura,
    }
);

impl pns_resolvers::resolvers::Config for Test {
    type Event = Event;

    type WeightInfo = ();

    type AccountIndex = u32;

    type RegistryChecker = TestChecker;

    type DomainHash = Hash;
}

impl crate::origin::Config for Test {
    type Event = Event;

    type WeightInfo = ();
}

pub struct TestChecker;

impl pns_resolvers::resolvers::RegistryChecker for TestChecker {
    type Hash = Hash;

    type AccountId = AccountId;

    fn check_node_useable(node: Self::Hash, owner: &Self::AccountId) -> bool {
        use crate::traits::Registrar as _;
        crate::nft::TokensByOwner::<Test>::contains_key((owner, 0, node))
            && Registrar::check_expires_useable(node).is_ok()
    }
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub const OFFICIAL_ACCOUNT: AccountId = 0;
pub const MANAGER_ACCOUNT: AccountId = 1;

pub const POOR_ACCOUNT: AccountId = 2;
pub const RICH_ACCOUNT: AccountId = 4;
pub const MONEY_ACCOUNT: AccountId = 5;

pub const BASE: Balance = 1_000_000_000_000;

// Build genesis storage according to the mock Test.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut genesis_storage = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    let registry_genesis = crate::registry::GenesisConfig::<Test> {
        official: Some(OFFICIAL_ACCOUNT),
        ..Default::default()
    };
    <crate::registry::GenesisConfig<Test> as frame_support::traits::GenesisBuild<Test>>::assimilate_storage(&registry_genesis,&mut genesis_storage).unwrap();

    let origin_genesis = crate::origin::GenesisConfig::<Test> {
        origins: vec![OFFICIAL_ACCOUNT, MANAGER_ACCOUNT],
    };

    <crate::origin::GenesisConfig<Test> as frame_support::traits::GenesisBuild<Test>>::assimilate_storage(&origin_genesis,&mut genesis_storage).unwrap();

    let nft_genesis = crate::nft::GenesisConfig::<Test> {
        tokens: vec![(
            OFFICIAL_ACCOUNT,
            Default::default(),
            (),
            vec![(
                OFFICIAL_ACCOUNT,
                Default::default(),
                Default::default(),
                DOT_BASENODE,
            )],
        )],
    };

    <crate::nft::GenesisConfig<Test> as frame_support::traits::GenesisBuild<Test>>::assimilate_storage(&nft_genesis,&mut genesis_storage).unwrap();

    let balances_genesis = pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (RICH_ACCOUNT, 500_000_000_000_000),
            (MONEY_ACCOUNT, 500_000_000_000_000),
        ],
    };

    <pallet_balances::GenesisConfig<Test> as frame_support::traits::GenesisBuild<Test>>::assimilate_storage(&balances_genesis,&mut genesis_storage).unwrap();

    let price_oracle_genesis = crate::price_oracle::GenesisConfig::<Test> {
        base_prices: [11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
        rent_prices: [11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
        deposit_prices: [11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
        init_rate: BASE,
    };

    <crate::price_oracle::GenesisConfig<Test> as frame_support::traits::GenesisBuild<Test>>::assimilate_storage(&price_oracle_genesis,&mut genesis_storage).unwrap();

    let registrar_genesis = crate::registrar::GenesisConfig::<Test> {
        infos: Default::default(),
        reserved_list: Default::default(),
    };

    <crate::registrar::GenesisConfig<Test> as frame_support::traits::GenesisBuild<Test>>::assimilate_storage(&registrar_genesis,&mut genesis_storage).unwrap();

    genesis_storage.into()
}

parameter_types! {
    pub const MaxMetadata: u32 = 15;
}

impl crate::nft::Config for Test {
    type ClassId = u32;

    type TokenId = Hash;

    type TotalId = u128;

    type ClassData = ();

    type TokenData = crate::registry::Record;

    type MaxClassMetadata = MaxMetadata;

    type MaxTokenMetadata = MaxMetadata;
}

impl crate::registry::Config for Test {
    type Event = Event;

    type WeightInfo = ();

    type Registrar = crate::registrar::Pallet<Test>;

    type ResolverId = u32;

    type ManagerOrigin = ManagerOrigin;
}

parameter_types! {
    pub const GracePeriod: BlockNumber = 90 * 24 * 60 * 60;
    pub const MinRegistrationDuration: Moment = 28 * 24 * 60 * 60;
    pub const DefaultCapacity: u32 = 20;
    pub const BaseNode: Hash = DOT_BASENODE;
}

pub type Moment = u64;

impl crate::registrar::Config for Test {
    type Event = Event;

    type ResolverId = u32;

    type Registry = crate::registry::Pallet<Test>;

    type Currency = pallet_balances::Pallet<Test>;

    type GracePeriod = GracePeriod;

    type DefaultCapacity = DefaultCapacity;

    type BaseNode = BaseNode;

    type WeightInfo = ();

    type MinRegistrationDuration = MinRegistrationDuration;

    type PriceOracle = crate::price_oracle::Pallet<Test>;

    type Moment = Moment;

    type NowProvider = pallet_timestamp::Pallet<Test>;

    type Official = crate::registry::Pallet<Test>;

    type ManagerOrigin = ManagerOrigin;

    type IsOpen = crate::origin::Pallet<Test>;
}

impl crate::price_oracle::Config for Test {
    type Event = Event;

    type Currency = pallet_balances::Pallet<Test>;

    type WeightInfo = ();

    type Moment = Moment;

    type ExchangeRate = TestRate;

    type ManagerOrigin = ManagerOrigin;
}

pub struct TestRate;

impl crate::traits::ExchangeRate for TestRate {
    type Balance = Balance;

    fn get_exchange_rate() -> Self::Balance {
        29580000
    }
}

impl crate::redeem_code::Config for Test {
    type Event = Event;

    type WeightInfo = ();

    type Registrar = crate::registrar::Pallet<Test>;

    type Moment = Moment;

    type Public = sp_runtime::testing::UintAuthorityId;

    type Signature = sp_runtime::testing::TestSignature;

    type Official = crate::registry::Pallet<Test>;

    type ManagerOrigin = ManagerOrigin;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 500;
    pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Test {
    type MaxLocks = MaxLocks;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
}
parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Test {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}
parameter_types! {
    pub const MaxAuthorities: u32 = 32;
}

impl pallet_aura::Config for Test {
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = MaxAuthorities;
}
