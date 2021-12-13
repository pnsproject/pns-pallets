use frame_support::{dispatch::Weight, parameter_types};
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
        Resolvers: pns_resolvers,
        Nft: crate::nft,
        Balances: pallet_balances,
        Timestamp: pallet_timestamp,
        Aura: pallet_aura,
    }
);

impl pns_resolvers::Config for Test {
    type Event = Event;

    type WeightInfo = TestWeightInfo;

    type AccountIndex = u32;

    type RegistryChecker = TestChecker;

    type DomainHash = Hash;
}

pub struct TestChecker;

impl pns_resolvers::traits::RegistryChecker for TestChecker {
    type Hash = Hash;

    type AccountId = AccountId;
    // TODO: 跨链验证
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
}

// Build genesis storage according to the mock Test.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}

pub struct TestWeightInfo;

impl pns_resolvers::WeightInfo for TestWeightInfo {
    fn set_text(content_len: usize) -> Weight {
        10 * content_len as Weight + 10_000
    }

    fn set_account() -> Weight {
        10_000
    }
}

impl crate::registry::WeightInfo for TestWeightInfo {
    fn set_approval_for_all() -> Weight {
        10_000
    }

    fn set_resolver() -> Weight {
        10_000
    }

    fn destroy() -> Weight {
        10_000
    }

    fn set_official() -> Weight {
        10_000
    }

    fn add_manger() -> Weight {
        10_000
    }

    fn remove_manger() -> Weight {
        10_000
    }
}

impl crate::registrar::WeightInfo for TestWeightInfo {
    fn mint_subname() -> Weight {
        10_000
    }

    fn register() -> Weight {
        10_000
    }

    fn renew() -> Weight {
        10_000
    }

    fn set_owner() -> Weight {
        10_000
    }

    fn reclaimed() -> Weight {
        10_000
    }

    fn add_blacklist() -> Weight {
        10_000
    }

    fn remove_blacklist() -> Weight {
        10_000
    }
}

impl crate::redeem_code::WeightInfo for TestWeightInfo {
    fn mint_redeem(len: Option<u32>) -> Weight {
        if let Some(len) = len {
            len as Weight * 10_000
        } else {
            10_000
        }
    }

    fn name_redeem() -> Weight {
        10_000
    }

    fn name_redeem_any() -> Weight {
        10_000
    }
}

impl crate::price_oracle::WeightInfo for TestWeightInfo {
    fn set_price() -> Weight {
        10_000
    }
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

    type WeightInfo = TestWeightInfo;

    type Registrar = crate::registrar::Pallet<Test>;

    type ResolverId = u32;
}

parameter_types! {
    pub const GracePeriod: BlockNumber = 90 * 24 * 60 * 60;
    pub const MinRegistrationDuration: Moment = 28 * 24 * 60 * 60;
    pub const DefaultCapacity: u32 = 20;
    pub const BaseNode: Hash = sp_core::H256([206, 21, 156, 243, 67, 128, 117, 125, 25, 50, 168, 228, 167, 78, 133, 232, 89, 87, 176, 167, 165, 45, 156, 86, 108, 10, 60, 141, 97, 51, 208, 247]);
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

    type WeightInfo = TestWeightInfo;

    type MinRegistrationDuration = MinRegistrationDuration;

    type PriceOracle = crate::price_oracle::Pallet<Test>;

    type Moment = Moment;

    type NowProvider = pallet_timestamp::Pallet<Test>;

    type Manager = crate::registry::Pallet<Test>;
}

parameter_types! {
    pub const MaximumLength: u8 = 10;
    pub const RateScale: Balance = 100_000;
}

impl crate::price_oracle::Config for Test {
    type Event = Event;

    type Currency = pallet_balances::Pallet<Test>;

    type MaximumLength = MaximumLength;

    type WeightInfo = TestWeightInfo;

    type Moment = Moment;

    type ExchangeRate = TestRate;

    type Manager = crate::registry::Pallet<Test>;

    type RateScale = RateScale;
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

    type WeightInfo = TestWeightInfo;

    type Registrar = crate::registrar::Pallet<Test>;

    type BaseNode = BaseNode;

    type Moment = Moment;

    type Public = sp_runtime::testing::UintAuthorityId;

    type Signature = sp_runtime::testing::TestSignature;

    type Manager = crate::registry::Pallet<Test>;
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

impl crate::traits::RedeemsGenerate for Test {
    type Moment = Moment;

    type Hash = Hash;

    type Pair = sp_core::ed25519::Pair;

    type Public = sp_core::ed25519::Public;

    type Signature = sp_core::ed25519::Signature;
}
