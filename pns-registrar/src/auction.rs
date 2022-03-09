pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::{BalanceOf, WeightInfo};
    use crate::traits::{Official, Registrar};
    use frame_support::traits::{Currency, EnsureOrigin, Get, ReservableCurrency};
    use frame_support::{pallet_prelude::*, Twox64Concat};
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{CheckedAdd, CheckedSub};
    use sp_runtime::ArithmeticError;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type WeightInfo: WeightInfo;

        type Currency: ReservableCurrency<Self::AccountId>;

        type Official: Official<AccountId = Self::AccountId>;

        #[pallet::constant]
        type MinimumPeriod: Get<Self::BlockNumber>;

        #[pallet::constant]
        type MaximumPeriod: Get<Self::BlockNumber>;

        type Token: Parameter
            + Member
            + MaybeSerializeDeserialize
            + core::fmt::Debug
            + sp_runtime::traits::MaybeDisplay
            + sp_runtime::traits::SimpleBitOps
            + Ord
            + Default
            + Copy
            + sp_runtime::traits::CheckEqual
            + sp_std::hash::Hash
            + AsRef<[u8]>
            + AsMut<[u8]>
            + sp_runtime::traits::MaybeMallocSizeOf
            + MaxEncodedLen;

        type ManagerOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

        type Registrar: Registrar<
            Hash = Self::Token,
            Balance = BalanceOf<Self>,
            AccountId = Self::AccountId,
        >;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type AuctionInfos<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::Token,
        Twox64Concat,
        T::BlockNumber,
        (T::AccountId, BalanceOf<T>),
    >;

    /// 正在进行的拍卖
    #[pallet::storage]
    pub type OnGoingAuction<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::BlockNumber, Twox64Concat, T::Token, ()>;

    /// 初始化拍卖
    #[pallet::storage]
    pub type AuctionInit<T: Config> = StorageMap<_, Twox64Concat, T::Token, T::BlockNumber>;

    // 已经是锁定的金额
    #[pallet::storage]
    pub type ReservedList<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::Token, Twox64Concat, T::AccountId, BalanceOf<T>>;

    #[pallet::storage]
    pub type Winners<T: Config> =
        StorageMap<_, Twox64Concat, T::Token, (T::AccountId, BalanceOf<T>, T::BlockNumber)>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// 拍卖已经结束，正在进行结算
        AuctionClosed,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::cancel_auction())]
        pub fn cancel_auction(origin: OriginFor<T>, token: T::Token) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn bid(caller: T::AccountId, token: T::Token, amount: BalanceOf<T>) -> DispatchResult {
            let now = frame_system::Pallet::<T>::block_number();
            if let Some(init_time) = AuctionInit::<T>::get(&token) {
                // 说明拍卖已经在进行中了，进行新的出价即可

                // 溢出已经在插入的时候校验过了
                let end = init_time + T::MaximumPeriod::get();

                // 确保处于拍卖阶段
                ensure!(now <= end, Error::<T>::AuctionClosed);

                if let Some(befor_reserved) = ReservedList::<T>::get(&token, &caller) {
                    let need_reserved = amount
                        .checked_sub(&befor_reserved)
                        .ok_or(ArithmeticError::Overflow)?;
                    T::Currency::reserve(&caller, need_reserved)?;
                } else {
                    T::Currency::reserve(&caller, amount)?;
                }

                AuctionInfos::<T>::insert(&token, &now, (&caller, amount));
            } else {
                // 说明改token没有被拍卖，需要创建一个新的拍卖
                T::Currency::reserve(&caller, amount)?;

                ReservedList::<T>::insert(&token, &caller, amount);

                // 确保未来不会溢出
                ensure!(
                    now.checked_add(&T::MaximumPeriod::get()).is_some(),
                    ArithmeticError::Overflow
                );

                AuctionInfos::<T>::insert(&token, &now, (&caller, amount));

                AuctionInit::<T>::insert(&token, &now);

                OnGoingAuction::<T>::insert(&now, &token, ());
            }

            Ok(())
        }

        pub fn handle_ending(now: T::BlockNumber) {
            now.checked_sub(&T::MaximumPeriod::get())
                .map(OnGoingAuction::<T>::iter_key_prefix)
                .map(|iter| {
                    iter.for_each(|token| {
                        // TODO: 错误处理
                        let res = Self::handle_one_ending(token, now);
                    })
                });
        }

        pub fn handle_one_ending(token: T::Token, now: T::BlockNumber) -> DispatchResult {
            let base_time = now - T::MaximumPeriod::get() + T::MinimumPeriod::get();
            // TODO: 差一个随机数生成的函数,范围在0到(max-min)之间
            let random = 0_u32;
            let real_end = base_time + random.into();

            for (bid_time, (bidder, bid_value)) in
                AuctionInfos::<T>::drain_prefix(&token).filter(|(time, _)| *time <= real_end)
            {
                if let Some((_, winner_value, winner_time)) = Winners::<T>::get(&token) {
                    if bid_value > winner_value
                        || (bid_value == winner_value && bid_time < winner_time)
                    {
                        Winners::<T>::insert(&token, (&bidder, bid_value, bid_time));
                    }
                } else {
                    Winners::<T>::insert(&token, (&bidder, bid_value, bid_time));
                }
            }

            if let Some((winner, winner_value, _)) = Winners::<T>::get(&token) {
                if let Some(reserved) = ReservedList::<T>::take(&token, &winner) {
                    T::Currency::unreserve(&winner, reserved);
                }

                let official = T::Official::get_official_account()?;

                T::Currency::transfer(
                    &winner,
                    &official,
                    winner_value,
                    frame_support::traits::ExistenceRequirement::AllowDeath,
                )?;

                T::Registrar::for_auction(token, winner, 0_u32.into(), 0_u32.into());
            }

            for (who, reserved) in ReservedList::<T>::drain_prefix(&token) {
                T::Currency::unreserve(&who, reserved);
            }

            Ok(())
        }
    }
}

use frame_support::dispatch::Weight;

pub trait WeightInfo {
    fn cancel_auction() -> Weight;
}

// TODO:
/// 延迟验证
///
/// 当前处理的交易太多的情况下，移交到后面的区块进行处理
pub struct DelayVrf;
