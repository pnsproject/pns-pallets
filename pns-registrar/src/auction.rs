pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

/// An enum which tracks the status of the auction system, and which phase it is in.
#[derive(PartialEq, Debug)]
pub enum AuctionStatus<BlockNumber> {
	/// We are in the traits::{DomainName, Registrar, Registry}ar, Registry}ar, Registry}on,
	/// collecting initial bids.
	StartingPeriod,
	/// An auction has not exist yet.
	AuctionNotExist,
	/// We are in the ending period of the auction, where we are taking snapshots of the winning
	/// bids. This state supports "sampling", where we may only take a snapshot every N blocks.
	/// In this case, the first number is the current sample number, and the second number
	/// is the sub-sample. i.e. for sampling every 20 blocks, the 25th block in the ending period
	/// will be `EndingPeriod(1, 5)`.
	EndingPeriod(BlockNumber, BlockNumber),
	/// We have completed the bidding process and are waiting for the VRF to return some acceptable
	/// randomness to select the winner. The number represents how many blocks we have been
	/// waiting.
	VrfDelay(BlockNumber),
}

impl<BlockNumber> AuctionStatus<BlockNumber> {
	/// Returns true if the auction is in any state other than `NotStarted`.
	pub fn is_in_progress(&self) -> bool {
		!matches!(self, Self::AuctionNotExist)
	}
	/// Return true if the auction is in the starting period.
	pub fn is_starting(&self) -> bool {
		matches!(self, Self::StartingPeriod)
	}
	/// Returns `Some(sample, sub_sample)` if the auction is in the `EndingPeriod`,
	/// otherwise returns `None`.
	pub fn is_ending(self) -> Option<(BlockNumber, BlockNumber)> {
		match self {
			Self::EndingPeriod(sample, sub_sample) => Some((sample, sub_sample)),
			_ => None,
		}
	}
	/// Returns true if the auction is in the `VrfDelay` period.
	pub fn is_vrf(&self) -> bool {
		matches!(self, Self::VrfDelay(_))
	}
}
/// `[bidder,amount]`
type WinnerData<T> = (<T as frame_system::Config>::AccountId, BalanceOf<T>);

#[frame_support::pallet]
pub mod pallet {
	use sp_std
::{collections::BTreeSet, vec::Vec};

	use super::*;
	use crate::{
		registry::{ClassData, TokenData},
		traits::{DomainName, Registrar, Registry},
	};
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, Randomness, ReservableCurrency},
	};
	use frame_system::{ensure_root, pallet_prelude::*};
	use sp_runtime::traits::{CheckedSub, One, Saturating, Zero};

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Currency: ReservableCurrency<Self::AccountId>;

		type ResolverId: Clone + Decode + Encode + Eq + PartialEq + core::fmt::Debug + Default;

		type Registry: Registry<
			AccountId = Self::AccountId,
			Context = (Self::ResolverId, u64, TokenData<Self::Hash>),
			Hash = Self::Hash,
			ClassData = ClassData,
		>;

		type Registrar: Registrar<
			Hash = Self::Hash,
			Duration = Self::BlockNumber,
			Balance = BalanceOf<Self>,
		>;

		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;

		#[pallet::constant]
		type Offical: Get<Self::AccountId>;

		#[pallet::constant]
		type EndingPeriod: Get<Self::BlockNumber>;

		#[pallet::constant]
		type MinimalAuctionPrice: Get<BalanceOf<Self>>;

		/// The length of each sample to take during the ending period.
		///
		/// `EndingPeriod` / `SampleLength` = Total # of Samples
		#[pallet::constant]
		type SampleLength: Get<Self::BlockNumber>;

		type WeightInfo: WeightInfo;
	}

	// 拍卖开始时间
	#[pallet::storage]
	#[pallet::getter(fn auction_info)]
	pub type AuctionInfo<T: Config> = StorageMap<_, Twox64Concat, T::Hash, T::BlockNumber>;

	#[pallet::storage]
	pub type OngoingAuctions<T: Config> = StorageValue<_, BTreeSet<T::Hash>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn reserved_amounts)]
	pub type ReservedAmounts<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::Hash, Twox64Concat, T::AccountId, BalanceOf<T>>;

	/// `node -> offset -> [bidder,bid_value]`
	#[pallet::storage]
	#[pallet::getter(fn winning)]
	pub type Winning<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::Hash, Twox64Concat, T::BlockNumber, WinnerData<T>>;

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// `[controller]`
		ControllerAdded(T::AccountId),
		/// `[start_time,node]`
		AuctionStarted(T::BlockNumber, T::Hash),
		/// `[node,offset]`hui
		WinningOffset(T::Hash, T::BlockNumber),
		/// `[bidder,extra_reserved,total_amount]`
		Reserved(T::AccountId, BalanceOf<T>, BalanceOf<T>),
		/// Funds were unreserved since bidder is no longer active. `[bidder, amount]`
		Unreserved(T::AccountId, BalanceOf<T>),
		/// `[bidder,node,amount]`
		BidAccepted(T::AccountId, T::Hash, BalanceOf<T>),
		/// `[node, winner, amount]`
		ReserveConfiscated(T::Hash, T::AccountId, BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		NameHasRegistered,
		AuctionNotExist,
		AuctionEnded,
		BidNotEnough,
		DomainBuildFailed,
		UnAnctionable,
		GetNodeFailed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			let mut weight = T::DbWeight::get().reads(1);
			let nodes = OngoingAuctions::<T>::get();

			for node in nodes {
				if let AuctionStatus::EndingPeriod(offset, _sub_sample) =
					Self::auction_status(n, node)
				{
					weight = weight.saturating_add(T::DbWeight::get().reads(1));
					if !Winning::<T>::contains_key(node, &offset) {
						weight = weight.saturating_add(T::DbWeight::get().writes(1));
						let winning_data = offset
							.checked_sub(&One::one())
							.and_then(|sub_one| Winning::<T>::get(node, sub_one))
							.expect("Winning data not found!");
						Winning::<T>::insert(node, offset, winning_data);
					}
				}

				// Check to see if an auction just ended.
				if let Some(winner_data) = Self::check_auction_end(n, node) {
					// Auction is ended now. We have the winning ranges and the lease period index
					// which acts as the offset. Handle it.
					Self::manage_auction_end(node, winner_data).unwrap_or_default();
					weight = weight.saturating_add(T::WeightInfo::on_initialize());
				}
			}

			weight
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[frame_support::transactional]
		#[pallet::weight(T::WeightInfo::bid())]
		pub fn bid(origin: OriginFor<T>, name: Vec<u8>, amount: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let domain = DomainName::<T::Hash, T::BlockNumber, BalanceOf<T>>::new(&name)
				.ok_or_else(|| Error::<T>::DomainBuildFailed)?;

			ensure!(T::Registrar::is_anctionable(&domain), Error::<T>::UnAnctionable);

			let node = domain.iter().last().ok_or_else(|| Error::<T>::GetNodeFailed)?;

			Self::handle_bid(who, node, amount)?;
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn do_new_auction(node: T::Hash, amount: BalanceOf<T>) -> DispatchResult {
			ensure!(amount > T::MinimalAuctionPrice::get(), Error::<T>::BidNotEnough);
			let now = frame_system::Pallet::<T>::block_number();

			AuctionInfo::<T>::insert(node, now);
			OngoingAuctions::<T>::mutate(|set| set.insert(node));

			Self::deposit_event(Event::<T>::AuctionStarted(now, node));

			Ok(())
		}
		fn auction_status(now: T::BlockNumber, node: T::Hash) -> AuctionStatus<T::BlockNumber> {
			let start = match AuctionInfo::<T>::get(node) {
				Some(start_time) => start_time,
				None => return AuctionStatus::AuctionNotExist,
			};
			let duration = match now.checked_sub(&start) {
				Some(sub_res) => sub_res,
				None => return AuctionStatus::StartingPeriod,
			};

			let ending_period = T::EndingPeriod::get();
			if duration < ending_period {
				let sample_length = T::SampleLength::get().max(One::one());
				let sample = duration / sample_length;
				let sub_sample = duration % sample_length;
				return AuctionStatus::EndingPeriod(sample, sub_sample)
			} else {
				return AuctionStatus::VrfDelay(duration - ending_period)
			}
		}

		fn handle_bid(bidder: T::AccountId, node: T::Hash, amount: BalanceOf<T>) -> DispatchResult {
			// 确保出价的域名尚未被注册
			ensure!(!T::Registry::is_registered(node), Error::<T>::NameHasRegistered);
			// 确保拍卖存在
			// 如果不存在直接创建就可以了
			let now = frame_system::Pallet::<T>::block_number();
			let auction_status = Self::auction_status(now, node);
			let offset = match auction_status {
				AuctionStatus::AuctionNotExist => {
					Self::do_new_auction(node, amount)?;
					Zero::zero()
				},
				AuctionStatus::StartingPeriod => Zero::zero(),
				AuctionStatus::EndingPeriod(o, _) => o,
				AuctionStatus::VrfDelay(_) => return Err(Error::<T>::AuctionEnded.into()),
			};

			let current_winning = Winning::<T>::get(node, offset).or_else(|| {
				offset
					.checked_sub(&One::one())
					.and_then(|sub_one| Winning::<T>::get(node, sub_one))
			});

			if current_winning.map_or(true, |last| amount > last.1) {
				let already_reserved = ReservedAmounts::<T>::get(node, &bidder).unwrap_or_default();

				if let Some(additional) = amount.checked_sub(&already_reserved) {
					T::Currency::reserve(&bidder, additional)?;
					// ...and record the amount reserved.
					ReservedAmounts::<T>::insert(node, &bidder, amount);

					Self::deposit_event(Event::<T>::Reserved(bidder.clone(), additional, amount));
				}
				let outgoing_winner = (bidder.clone(), amount);
				// todo：这里不知道有没有用
				// core::mem::swap(&mut current_winning, &mut outgoing_winner);

				// if let Some((who, node, amount)) = outgoing_winner {
				// 	// 这里似乎没有必要
				// 	// 波卡里通过判断拍卖的状态是否在未结束状态，来退还不活跃用户的资金
				// 	// 这个功能有必要，但是需要优化，判断哪些是不积极的竞标者
				// 	if auction_status.is_starting() &&
				// 		current_winning.as_ref().map(|&(ref other, other_node, _)| {
				// 			other != &who || other_node != node
				// 		}).unwrap_or_default() {
				// 		if let Some(amount) = ReservedAmounts::<T>::take(node, &who) {
				// 			// It really should be reserved; there's not much we can do here on
				// 			// fail.
				// 			let err_amt = T::Currency::unreserve(&who, amount);
				// 			debug_assert!(err_amt.is_zero());
				// 			Self::deposit_event(Event::<T>::Unreserved(who, amount));
				// 		}
				// 	}
				// }

				Winning::<T>::insert(node, offset, outgoing_winner);
				Self::deposit_event(Event::<T>::BidAccepted(bidder, node, amount));
			}
			Ok(())
		}

		fn check_auction_end(now: T::BlockNumber, node: T::Hash) -> Option<WinnerData<T>> {
			AuctionInfo::<T>::get(node).and_then(|start_time| {
				let ending_period = T::EndingPeriod::get();
				let late_end = start_time.saturating_add(ending_period);
				let is_ended = now >= late_end;
				if is_ended {
					let (raw_offset, known_since) = T::Randomness::random(&b"pns_auction"[..]);
					if late_end <= known_since {
						// Our random seed was known only after the auction ended. Good to use.
						let raw_offset_block_number = <T::BlockNumber>::decode(
							&mut raw_offset.as_ref(),
						)
						.expect("secure hashes should always be bigger than the block number; qed");
						let offset = (raw_offset_block_number % ending_period) /
							T::SampleLength::get().max(One::one());

						Self::deposit_event(Event::<T>::WinningOffset(node, offset));

						let res = Winning::<T>::get(node, offset);
						Winning::<T>::remove_prefix(node, None);
						AuctionInfo::<T>::remove(node);
						return res
					}
				}
				None
			})
		}

		fn manage_auction_end(node: T::Hash, winner_data: WinnerData<T>) -> DispatchResult {
			// 释放之前收缴的货币
			for (bidder, amount) in ReservedAmounts::<T>::drain_prefix(node) {
				T::Currency::unreserve(&bidder, amount);
			}

			let (winner, bid_value) = winner_data;

			let offical = T::Offical::get();

			T::Currency::transfer(
				&winner,
				&offical,
				bid_value,
				frame_support::traits::ExistenceRequirement::KeepAlive,
			)?;

			T::Registrar::for_auction_set_expires(node);

			T::Registry::mint(
				winner,
				ClassData::Auction,
				node,
				(Default::default(), 10, TokenData::Root(node)),
			)?;
			Ok(())
		}
	}
}
use frame_support::dispatch::Weight;
pub trait WeightInfo {
	fn new_auction() -> Weight;
	fn bid() -> Weight;
	fn cancel_auction() -> Weight;
	fn on_initialize() -> Weight;
}
