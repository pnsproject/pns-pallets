use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
    dispatch::{DispatchResult, Weight},
    ensure,
};
use pallet::*;
use scale_info::TypeInfo;

/// 用来追踪拍卖的进展
#[derive(PartialEq, Debug, Decode, Encode, Clone, TypeInfo, MaxEncodedLen)]
pub enum AuctionState {
    OngoingPeriod,
}

type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

#[derive(Debug, Decode, Encode, Clone, TypeInfo, MaxEncodedLen)]
pub struct BidInfo<Balance, BlockNumber, AccountId> {
    amount: Balance,
    sample_start: BlockNumber,
    bidder: AccountId,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use codec::{Decode, Encode, EncodeLike, MaxEncodedLen};
    use frame_support::pallet_prelude::StorageDoubleMap;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{Currency, ReservableCurrency};
    use frame_support::Twox64Concat;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{One, Zero};
    use sp_std::vec::Vec;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Currency: ReservableCurrency<Self::AccountId>;

        type Preparation: Preparation<Token = Self::Token, Balance = BalanceOf<Self>>;

        type Token: Decode
            + Encode
            + MaxEncodedLen
            + EncodeLike
            + TypeInfo
            + Clone
            + Eq
            + PartialEq
            + core::fmt::Debug;

        #[pallet::constant]
        type MinimumPeriod: Get<Self::BlockNumber>;

        #[pallet::constant]
        type EndingPeriod: Get<Self::BlockNumber>;

        /// The length of each sample to take during the ending period.
        ///
        /// `EndingPeriod` / `SampleLength` = Total # of Samples
        #[pallet::constant]
        type SampleLength: Get<Self::BlockNumber>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// `[controller]`
        ControllerAdded(T::AccountId),
        /// `[start_time,node]`
        AuctionStarted {
            start_time: T::BlockNumber,
            token: T::Token,
            bidder: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// `[node,offset]`hui
        WinningOffset(T::Token, T::BlockNumber),
        /// `[bidder,extra_reserved,total_amount]`
        Reserved(T::AccountId, BalanceOf<T>, BalanceOf<T>),
        /// Funds were unreserved since bidder is no longer active. `[bidder, amount]`
        Unreserved(T::AccountId, BalanceOf<T>),
        /// `[bidder,node,amount]`
        BidAccepted(T::AccountId, T::Token, BalanceOf<T>),
        /// `[node, winner, amount]`
        ReserveConfiscated(T::Token, T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        BidNotEnough,
        DomainBuildFailed,
        UnAnctionable,
        GetNodeFailed,
        /// 拍卖不存在
        AuctionNotExist,
        /// 资金不足
        Underfunded,
        /// 拍卖正在进行，无法创建拍卖，只能出价
        AuctionInProgress,
    }

    #[pallet::storage]
    pub type BiddersInfo<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::Token, Twox64Concat, T::AccountId, BalanceOf<T>>;

    #[pallet::storage]
    pub type AmountInfo<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::Token, Twox64Concat, T::BlockNumber, BalanceOf<T>>;

    #[pallet::storage]
    pub type AuctionStatus<T: Config> = StorageMap<_, Twox64Concat, T::Token, AuctionState>;

    /// start time -> token
    #[pallet::storage]
    pub type AuctionHandle<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::BlockNumber, Twox64Concat, T::Token, ()>;

    /// [token -> offset] -> bid info
    #[pallet::storage]
    pub type OngoingAuctions<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::Token,
        Twox64Concat,
        T::BlockNumber,
        BidInfo<BalanceOf<T>, T::BlockNumber, T::AccountId>,
    >;

    #[pallet::storage]
    pub type GlobalBidInfo<T: Config> =
        StorageMap<_, Twox64Concat, T::Token, BidInfo<BalanceOf<T>, T::BlockNumber, T::AccountId>>;

    impl<T: Config> Pallet<T> {
        #[cfg_attr(
            not(feature = "runtime-benchmarks"),
            frame_support::require_transactional
        )]
        fn new_auction(
            // 可拍卖的token
            token: T::Token,
            // token的起拍价
            minimum_amount: BalanceOf<T>,
            // 出价人
            bidder: T::AccountId,
        ) -> DispatchResult {
            // 确保出价人有足够的金钱创建一个新的拍卖
            ensure!(
                T::Currency::can_reserve(&bidder, minimum_amount),
                Error::<T>::Underfunded
            );
            // 确保当前的token不处于拍卖阶段
            ensure!(
                !AuctionStatus::<T>::contains_key(&token),
                Error::<T>::AuctionInProgress
            );

            // 存储出价
            T::Currency::reserve(&bidder, minimum_amount)?;

            let now = frame_system::Pallet::<T>::block_number();

            // 初始化Ongoing
            OngoingAuctions::<T>::insert(
                &token,
                T::BlockNumber::zero(),
                BidInfo {
                    bidder: bidder.clone(),
                    sample_start: now,
                    amount: minimum_amount,
                },
            );

            // 创建 auction handle
            AuctionHandle::<T>::insert(&now, &token, ());

            Self::deposit_event(Event::<T>::AuctionStarted {
                start_time: now,
                token,
                bidder,
                amount: minimum_amount,
            });

            Ok(())
        }
        #[cfg_attr(
            not(feature = "runtime-benchmarks"),
            frame_support::require_transactional
        )]
        fn handle_bid(
            bidder: T::AccountId,
            token: T::Token,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            // 确保当前的token处于拍卖阶段
            ensure!(
                AuctionStatus::<T>::contains_key(&token),
                Error::<T>::AuctionNotExist
            );

            let ongoing = OngoingAuctions::<T>::iter_prefix(&token);

            if let Some((offset, bid_info)) = ongoing.last() {
                let now = frame_system::Pallet::<T>::block_number();
                let sub = now - bid_info.sample_start;
                let sample_length = T::SampleLength::get();

                match sub.cmp(&sample_length) {
                    core::cmp::Ordering::Less => {}
                    core::cmp::Ordering::Equal => {
                        let global_bid_info = GlobalBidInfo::<T>::get(&token);

                        OngoingAuctions::<T>::insert(
                            &token,
                            offset + T::BlockNumber::one(),
                            BidInfo {
                                amount,
                                sample_start: now,
                                bidder,
                            },
                        );
                    }
                    core::cmp::Ordering::Greater => {}
                }
            }

            Ok(())
        }
        #[cfg_attr(
            not(feature = "runtime-benchmarks"),
            frame_support::require_transactional
        )]
        fn handle_ending(
            bidder: T::AccountId,
            token: T::Token,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            Ok(())
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::hello())]
        pub fn hello(origin: OriginFor<T>) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            Ok(())
        }
    }
}

pub trait WeightInfo {
    fn hello() -> Weight;
}

pub trait Preparation {
    type Token;
    type Balance;

    fn is_anctionable(token: &Self::Token) -> bool;
    fn min_bid_amount(len: usize) -> Self::Balance;
}
