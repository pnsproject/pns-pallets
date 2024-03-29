//! # Redeem Code
//!
//! This module is an implementation of the functionality
//! related to redemption codes.
//!
//! But the current implementation has a fatal problem.
//!
//! - When hackers submit transactions with their own accounts
//! by intercepting transactions submitted by other users before
//! other users successfully register, they may intercept the
//! domain name that can be redeemed by obtaining that redemption code.
//!
//! Therefore this part of the code should be re-implemented.
//!
//! ## Introduction
//!
//! This module has the function to redeem the redemption code,
//! and the function to generate the `nouce` needed to redeem the code.
//!
//! ### Module functions
//!
//! - `mint_redeem` - generates `nouce` for the specified range
//! - `name_redeem` - redeem the specified domain
//! - `name_redeem_any` - redeem any registrable domain name above a certain length (a certain length currently is 10 digits)
//!
//! All the above methods require manager privileges in `pnsOrigin`.

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::traits::{Available, Label, Official, Registrar};
    use codec::EncodeLike;
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::EnsureOrigin};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_runtime::traits::{AtLeast32Bit, IdentifyAccount, Verify};
    use sp_std::vec::Vec;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WeightInfo: WeightInfo;

        type Registrar: Registrar<AccountId = Self::AccountId, Moment = Self::Moment>;

        type Moment: AtLeast32Bit
            + Parameter
            + Default
            + Copy
            + MaxEncodedLen
            + MaybeSerializeDeserialize;

        type Public: TypeInfo
            + Decode
            + Encode
            + EncodeLike
            + MaybeSerializeDeserialize
            + core::fmt::Debug
            + IdentifyAccount<AccountId = Self::AccountId>;

        type Signature: Decode
            + Verify<Signer = Self::Public>
            + codec::Codec
            + EncodeLike
            + Clone
            + Eq
            + core::fmt::Debug
            + TypeInfo;

        type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        type Official: Official<AccountId = Self::AccountId>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// redeem code
    #[pallet::storage]
    pub type Redeems<T> = StorageMap<_, Twox64Concat, u32, ()>;

    #[pallet::genesis_config]
    #[cfg_attr(feature = "std", derive(Default))]
    pub struct GenesisConfig {
        /// (`start`,`end`)
        pub redeems: Option<(u32, u32)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            if let Some((start, end)) = self.redeems {
                let mut nouce = start;

                while nouce < end {
                    Redeems::<T>::insert(nouce, ());
                    nouce += 1;
                }
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// When the redemption code is used, it will be logged.
        RedeemCodeUsed {
            code: T::Signature,
            node: pns_types::DomainHash,
            to: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The `start` you entered is greater than or equal to `end`, which is an invalid range.
        RangeInvaild,
        /// The label you entered is not parsed properly, maybe there are illegal characters in your label.
        ParseLabelFailed,
        ///This is an internal error.
        ///
        /// The code signer entered does not match the expected one.
        /// Are you sure you are getting this error on the official PNS web page?
        ///
        /// If so, you can contact us and we will help you to solve this problem.
        InvalidSignature,
        /// The redemption code has already been used.
        RedeemsHasBeenUsed,
        /// The length of the domain name you entered does not match the
        /// requirements of this redemption code.
        LabelLenInvalid,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// This is a Root method which is used to create the nouce needed to redeem the code.
        ///
        /// Ensure: start < end
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::mint_redeem(end.checked_sub(*start).unwrap_or_default()))]
        pub fn mint_redeem(origin: OriginFor<T>, start: u32, end: u32) -> DispatchResult {
            let _who = T::ManagerOrigin::ensure_origin(origin)?;

            ensure!(start < end, Error::<T>::RangeInvaild);

            let mut nouce = start;

            while nouce <= end {
                Redeems::<T>::insert(nouce, ());
                nouce += 1;
            }

            Ok(())
        }
        /// This is an interface to the PNS front-end.
        ///
        /// Although you can also call it, but not through
        /// the `redemption code` to call the interface.
        ///
        /// The PNS front-end gets the `name`,`duration`,`nouce` and `code` from
        /// our central server through the `redemption code`,
        /// and then calls the interface.
        ///
        /// Ensure: The length of name needs to be greater than 3.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::name_redeem(name.len() as u32))]
        #[frame_support::transactional]
        pub fn name_redeem(
            origin: OriginFor<T>,
            name: Vec<u8>,
            duration: T::Moment,
            nouce: u32,
            code: T::Signature,
            owner: T::AccountId,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            ensure!(
                Redeems::<T>::contains_key(nouce),
                Error::<T>::RedeemsHasBeenUsed
            );

            let (label, _) = Label::new_with_len(&name).ok_or(Error::<T>::ParseLabelFailed)?;

            let label_node = label.node;
            let data = (label_node, duration, nouce).encode();

            let signer = T::Official::get_official_account()?;

            ensure!(
                code.verify(&data[..], &signer),
                Error::<T>::InvalidSignature
            );

            let node = label.encode_with_node(&T::Registrar::basenode());

            T::Registrar::for_redeem_code(name, owner.clone(), duration, label)?;

            Redeems::<T>::remove(nouce);

            Self::deposit_event(Event::<T>::RedeemCodeUsed {
                code,
                node,
                to: owner,
            });

            Ok(())
        }

        /// This is an interface to the PNS front-end.
        ///
        /// The PNS front-end gets `duration`, `nouce` and `code`
        /// from our central server via the redemption code,
        /// and gets `name` from the user, then calls this interface.
        ///
        /// NOTE: The front-end should check if the name is legal
        /// or occupied when it is called.
        ///
        /// Ensure: The length of name needs to be greater than 10.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::name_redeem_any(name.len() as u32))]
        #[frame_support::transactional]
        pub fn name_redeem_any(
            origin: OriginFor<T>,
            name: Vec<u8>,
            duration: T::Moment,
            nouce: u32,
            code: T::Signature,
            owner: T::AccountId,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            ensure!(
                Redeems::<T>::contains_key(nouce),
                Error::<T>::RedeemsHasBeenUsed
            );

            let (label, label_len) =
                Label::new_with_len(&name).ok_or(Error::<T>::ParseLabelFailed)?;

            ensure!(label_len.is_registrable(), Error::<T>::LabelLenInvalid);

            let data = (duration, nouce).encode();

            let signer = T::Official::get_official_account()?;

            ensure!(
                code.verify(&data[..], &signer),
                Error::<T>::InvalidSignature
            );

            let node = label.encode_with_node(&T::Registrar::basenode());

            T::Registrar::for_redeem_code(name, owner.clone(), duration, label)?;

            Redeems::<T>::remove(nouce);

            Self::deposit_event(Event::<T>::RedeemCodeUsed {
                code,
                node,
                to: owner,
            });

            Ok(())
        }
    }
}

use frame_support::dispatch::Weight;

use crate::traits::{LABEL_MIN_LEN, MIN_REGISTRABLE_LEN};

pub trait WeightInfo {
    fn mint_redeem(len: u32) -> Weight;
    fn name_redeem(len: u32) -> Weight {
        Self::create_label(len - LABEL_MIN_LEN as u32)
            + Self::for_redeem_code(len - LABEL_MIN_LEN as u32)
            + Self::name_redeem_min()
    }
    fn name_redeem_any(len: u32) -> Weight {
        Self::create_label(len - MIN_REGISTRABLE_LEN as u32)
            + Self::for_redeem_code(len - MIN_REGISTRABLE_LEN as u32)
            + Self::name_redeem_any_min()
    }
    fn create_label(len: u32) -> Weight;
    fn for_redeem_code(len: u32) -> Weight;
    fn name_redeem_min() -> Weight;
    fn name_redeem_any_min() -> Weight;
}

impl WeightInfo for () {
    fn mint_redeem(_len: u32) -> Weight {
        Weight::zero()
    }

    fn create_label(_len: u32) -> Weight {
        Weight::zero()
    }

    fn for_redeem_code(_len: u32) -> Weight {
        Weight::zero()
    }

    fn name_redeem_min() -> Weight {
        Weight::zero()
    }

    fn name_redeem_any_min() -> Weight {
        Weight::zero()
    }
}
