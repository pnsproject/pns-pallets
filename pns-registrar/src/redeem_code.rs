pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::traits::{Available, Label, Registrar};
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_runtime::{
        traits::{AtLeast32BitUnsigned, Verify},
        AnySignature,
    };
    use sp_std::vec::Vec;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type WeightInfo: WeightInfo;

        type Registrar: Registrar<
            AccountId = Self::AccountId,
            Hash = Self::Hash,
            Duration = Self::Moment,
        >;

        type Moment: Clone
            + Copy
            + Decode
            + Encode
            + Eq
            + PartialEq
            + core::fmt::Debug
            + Default
            + TypeInfo
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize;

        #[pallet::constant]
        type BaseNode: Get<Self::Hash>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// redeem code
    #[pallet::storage]
    pub type Redeems<T> = StorageMap<_, Twox64Concat, u32, ()>;
    /// Official Public
    #[pallet::storage]
    pub type OfficialSigner<T: Config> = StorageValue<_, sp_core::sr25519::Public, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub official_signer: Option<sp_core::sr25519::Public>,
        /// [`start`,`end`]
        pub redeems: Option<(u32, u32)>,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            GenesisConfig {
                official_signer: None,
                redeems: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            if let Some(signer) = self.official_signer {
                OfficialSigner::<T>::put(signer);
            }
            if let Some((start, end)) = self.redeems {
                let mut nouce = start;

                while nouce < end {
                    Redeems::<T>::insert(nouce, ());
                    nouce = nouce + 1;
                }
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// When the redemption code is used, it will be logged.
        /// [`code`,`node`,`to`]
        RedeemCodeUsed(Vec<u8>, T::Hash, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The `start` you entered is greater than or equal to `end`, which is an invalid range.
        RangeInvaild,
        /// The label you entered is not parsed properly, maybe there are illegal characters in your label.
        ParseLabelFailed,
        /// This is an internal error.
        ///
        /// The input code signature failed to be parsed,
        /// maybe you should try calling this transaction again.
        InputCodeParseFailed,
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
        #[pallet::weight(T::WeightInfo::mint_redeem(end.checked_sub(*start)))]
        pub fn mint_redeem(origin: OriginFor<T>, start: u32, end: u32) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(start < end, Error::<T>::RangeInvaild);

            let mut nouce = start;

            while nouce < end {
                Redeems::<T>::insert(nouce, ());
                nouce = nouce + 1;
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
        #[pallet::weight(T::WeightInfo::name_redeem())]
        pub fn name_redeem(
            origin: OriginFor<T>,
            name: Vec<u8>,
            duration: T::Moment,
            nouce: u32,
            code: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let (label, _) =
                Label::<T::Hash>::new(&name).ok_or_else(|| Error::<T>::ParseLabelFailed)?;

            let label_node = label.node;
            let data = (label_node, duration, nouce).encode();

            let mut signature_input = &code[..];

            let signature = AnySignature::decode(&mut signature_input)
                .map_err(|_| Error::<T>::InputCodeParseFailed)?;

            let signer = OfficialSigner::<T>::get();

            ensure!(
                signature.verify(&data[..], &signer),
                Error::<T>::InvalidSignature
            );

            ensure!(
                Redeems::<T>::contains_key(nouce),
                Error::<T>::RedeemsHasBeenUsed
            );

            T::Registrar::for_redeem_code(name, who.clone(), duration, label)?;

            Redeems::<T>::remove(nouce);

            Self::deposit_event(Event::<T>::RedeemCodeUsed(code, label_node, who));

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
        #[pallet::weight(T::WeightInfo::name_redeem_any())]
        pub fn name_redeem_any(
            origin: OriginFor<T>,
            name: Vec<u8>,
            duration: T::Moment,
            nouce: u32,
            code: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let (label, label_len) =
                Label::<T::Hash>::new(&name).ok_or_else(|| Error::<T>::ParseLabelFailed)?;

            ensure!(label_len.is_registrable(), Error::<T>::LabelLenInvalid);

            let data = (duration, nouce).encode();

            let mut signature_input = &code[..];

            let signature = AnySignature::decode(&mut signature_input)
                .map_err(|_| Error::<T>::InputCodeParseFailed)?;

            let signer = OfficialSigner::<T>::get();

            ensure!(
                signature.verify(&data[..], &signer),
                Error::<T>::InvalidSignature
            );

            ensure!(
                Redeems::<T>::contains_key(nouce),
                Error::<T>::RedeemsHasBeenUsed
            );

            T::Registrar::for_redeem_code(name, who, duration, label)?;

            Redeems::<T>::remove(nouce);

            Ok(())
        }
    }
}

use frame_support::dispatch::Weight;

pub trait WeightInfo {
    fn mint_redeem(len: Option<u32>) -> Weight;
    fn name_redeem() -> Weight;
    fn name_redeem_any() -> Weight;
}
