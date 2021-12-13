pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::traits::{Available, EnsureManager, Label, Registrar};
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_core::Pair;
    use sp_runtime::traits::AtLeast32BitUnsigned;
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

        type Pair: Pair<Public = Self::Public, Signature = Self::Signature>;

        type Public: Clone
            + sp_core::Public
            + core::hash::Hash
            + TypeInfo
            + Decode
            + Encode
            + codec::EncodeLike
            + MaybeSerializeDeserialize
            + Eq
            + PartialEq
            + core::fmt::Debug;

        type Signature: AsRef<[u8]> + Decode;

        type Manager: EnsureManager<AccountId = Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// redeem code
    #[pallet::storage]
    pub type Redeems<T> = StorageMap<_, Twox64Concat, u32, ()>;
    /// Official Public
    #[pallet::storage]
    pub type OfficialSigner<T: Config> = StorageValue<_, T::Public, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub official_signer: Option<T::Public>,
        /// [`start`,`end`]
        pub redeems: Option<(u32, u32)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                official_signer: None,
                redeems: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            if let Some(signer) = self.official_signer.as_ref() {
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
            let who = ensure_signed(origin)?;
            T::Manager::ensure_manager(who)?;

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
        #[frame_support::transactional]
        pub fn name_redeem(
            origin: OriginFor<T>,
            name: Vec<u8>,
            duration: T::Moment,
            nouce: u32,
            code: Vec<u8>,
            owner: T::AccountId,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            ensure!(
                Redeems::<T>::contains_key(nouce),
                Error::<T>::RedeemsHasBeenUsed
            );

            let (label, _) =
                Label::<T::Hash>::new(&name).ok_or_else(|| Error::<T>::ParseLabelFailed)?;

            let label_node = label.node;
            let data = (label_node, duration, nouce).encode();

            let mut signature_input = &code[..];

            let signature = T::Signature::decode(&mut signature_input)
                .map_err(|_| Error::<T>::InputCodeParseFailed)?;

            let signer = OfficialSigner::<T>::get();

            ensure!(
                T::Pair::verify(&signature, &data[..], &signer),
                Error::<T>::InvalidSignature
            );

            let node = label.encode_with_basenode(T::BaseNode::get());

            T::Registrar::for_redeem_code(name, owner.clone(), duration, label)?;

            Redeems::<T>::remove(nouce);

            Self::deposit_event(Event::<T>::RedeemCodeUsed(code, node, owner));

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
        #[frame_support::transactional]
        pub fn name_redeem_any(
            origin: OriginFor<T>,
            name: Vec<u8>,
            duration: T::Moment,
            nouce: u32,
            code: Vec<u8>,
            owner: T::AccountId,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            ensure!(
                Redeems::<T>::contains_key(nouce),
                Error::<T>::RedeemsHasBeenUsed
            );

            let (label, label_len) =
                Label::<T::Hash>::new(&name).ok_or_else(|| Error::<T>::ParseLabelFailed)?;

            ensure!(label_len.is_registrable(), Error::<T>::LabelLenInvalid);

            let data = (duration, nouce).encode();

            let mut signature_input = &code[..];

            let signature = T::Signature::decode(&mut signature_input)
                .map_err(|_| Error::<T>::InputCodeParseFailed)?;

            let signer = OfficialSigner::<T>::get();

            ensure!(
                T::Pair::verify(&signature, &data[..], &signer),
                Error::<T>::InvalidSignature
            );

            let node = label.encode_with_basenode(T::BaseNode::get());

            T::Registrar::for_redeem_code(name, owner.clone(), duration, label)?;

            Redeems::<T>::remove(nouce);

            Self::deposit_event(Event::<T>::RedeemCodeUsed(code, node, owner));

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
