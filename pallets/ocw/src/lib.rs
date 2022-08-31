#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	//! A demonstration of an offchain worker that sends onchain callbacks
	use core::{convert::TryInto};
	use frame_support::pallet_prelude::*;
	use frame_system::{
		self as system,
		pallet_prelude::*,
		offchain::{
			AppCrypto, CreateSignedTransaction, SubmitTransaction,
		},
	};
	use sp_core::{crypto::KeyTypeId};
	use sp_runtime::{
		traits::BlockNumberProvider,
		transaction_validity::{
			InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
		},
	};
	use sp_std::str;

	/// Defines application identifier for crypto keys of this module.
	///
	/// Every module that deals with signatures needs to declare its unique identifier for
	/// its crypto keys.
	/// When an offchain worker is signing transactions it's going to request keys from type
	/// `KeyTypeId` via the keystore to sign the transaction.
	/// The keys can be inserted manually via RPC (see `author_insertKey`).
	pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"demo");
	/// The type to sign and send transactions.
	const UNSIGNED_TXS_PRIORITY: u64 = 100;

	/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrapper.
	/// We can utilize the supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment
	/// them with the pallet-specific identifier.
	pub mod crypto {
		use crate::KEY_TYPE;
		use sp_core::sr25519::Signature as Sr25519Signature;
		use sp_std::prelude::*;
		use sp_runtime::{
			app_crypto::{app_crypto, sr25519},
			traits::Verify, MultiSignature, MultiSigner
		};

		app_crypto!(sr25519, KEY_TYPE);

		pub struct TestAuthId;
		// implemented for runtime
		impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
			type RuntimeAppPublic = Public;
			type GenericSignature = sp_core::sr25519::Signature;
			type GenericPublic = sp_core::sr25519::Public;
		}

		// implemented for mock runtime in test
		impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
		for TestAuthId
		{
			type RuntimeAppPublic = Public;
			type GenericSignature = sp_core::sr25519::Signature;
			type GenericPublic = sp_core::sr25519::Public;
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> + pallet_brand_membership::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The overarching dispatch call type.
		type Call: From<Call<Self>>;
		/// The identifier type for an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
	
		/// Number of blocks of cooldown after unsigned transaction is included.
		///
		/// This ensures that we only accept unsigned transactions once, every `UnsignedInterval`
		/// blocks.
		#[pallet::constant]
		type UnsignedInterval: Get<Self::BlockNumber>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn next_unsigned_at)]
	pub(super) type NextUnsignedAt<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {

	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		// Error returned when not sure which ocw function to executed
		UnknownOffchainMux,

		// Error returned when making signed transactions in off-chain worker
		NoLocalAcctForSigning,
		OffchainSignedTxError,

		// Error returned when making unsigned transactions in off-chain worker
		OffchainUnsignedTxError,

		// Error returned when making unsigned transactions with signed payloads in off-chain worker
		OffchainUnsignedTxSignedPayloadError,

		// Error returned when fetching github info
		HttpFetchingError,
		DeserializeToObjError,
		DeserializeToStrError,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Offchain Worker entry point.
		///
		/// By implementing `fn offchain_worker` you declare a new offchain worker.
		/// This function will be called when the node is fully synced and a new best block is
		/// succesfuly imported.
		/// Note that it's not guaranteed for offchain workers to run on EVERY block, there might
		/// be cases where some blocks are skipped, or for some the worker runs twice (re-orgs),
		/// so the code should be able to handle that.
		/// You can use `Local Storage` API to coordinate runs of the worker.
		fn offchain_worker(block_number: T::BlockNumber) {

			log::info!("------------Pallet-ocw!------------");

			// worker will be called after a day
			const TX_TYPES: u32 = 144000;
			let modu = block_number.try_into().map_or(TX_TYPES, |bn: usize| (bn as u32) % TX_TYPES);
			let result = match modu {
			// helper function
				0 => Self::offchain_unsigned_tx(block_number),
				_ => Err(Error::<T>::UnknownOffchainMux),
			};

			if let Err(e) = result {
				log::error!("offchain_worker error: {:?}", e);
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		/// Validate unsigned call to this module.
		///
		/// By default unsigned transactions are disallowed, but implementing the validator
		/// here we make sure that some particular calls (the ones produced by offchain worker)
		/// are being whitelisted and marked as valid.
		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			let valid_tx = |provide| ValidTransaction::with_tag_prefix("ocw")
				.priority(UNSIGNED_TXS_PRIORITY)
				.and_provides([&provide])
				.longevity(3)
				.propagate(true)
				.build();

			match call {
				Call::submit_number_unsigned { number: _number } => valid_tx(b"submit_number_unsigned".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10000)]
		pub fn submit_number_unsigned(origin: OriginFor<T>, number: u64) -> DispatchResult {
			ensure_none(origin)?;
			log::info!("submit_number_unsigned: {}", number);

			pallet_brand_membership::Pallet::<T>::check_expire_nft()?;

			let current_block = <system::Pallet<T>>::block_number();
			<NextUnsignedAt<T>>::put(current_block + T::UnsignedInterval::get());
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn offchain_unsigned_tx(block_number: T::BlockNumber) -> Result<(), Error<T>> {
			let number: u64 = block_number.try_into().unwrap_or(0);
			let call = Call::submit_number_unsigned { number };

			// `submit_unsigned_transaction` returns a type of `Result<(), ()>`
			//   ref: https://substrate.dev/rustdocs/v2.0.0/frame_system/offchain/struct.SubmitTransaction.html#method.submit_unsigned_transaction
			SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
			.map_err(|_| {
				log::error!("Failed in offchain_unsigned_tx");
				<Error<T>>::OffchainUnsignedTxError
			})
		}
	}
}