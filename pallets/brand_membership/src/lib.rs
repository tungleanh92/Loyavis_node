#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::{Scale},
		traits::{tokens::ExistenceRequirement, Currency, Randomness, ReservableCurrency, Time, Len},
		transactional, require_transactional,
		sp_runtime::traits::Saturating,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::{TypeInfo, StaticTypeInfo};
	use sp_io::hashing::blake2_128;
	use sp_std::vec::Vec;
	use sp_runtime::SaturatedConversion;
	use pallet_brand_token::BrandTransferToken;

	#[cfg(feature = "std")]
	use frame_support::serde::{Deserialize, Serialize};

	// brand token not native token, fix later
	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct NFTCollection<Account, Balance, Time> {
		pub title: Vec<u8>,
		pub description: Option<Vec<u8>>,
		pub creator: Account,
		pub deposit: Balance,
		pub expire: u8,
		pub created_at: Time
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct NonFungibleToken<Account, Balance, Time> {
		pub id: [u8; 16],
		pub title: Vec<u8>,
		pub description: Option<Vec<u8>>,
		pub media: Vec<u8>, // URI to associated media, preferably to decentralized, content-addressed storage
		pub creator: Account,
		pub owner: Account,
		pub collection_id: [u8; 16],
		pub deposit: Balance,
		pub price: Balance,
		pub expire: u8,
		pub created_at: Time,
		pub renew_time: Time,
		pub renew_fee: Balance,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_brand_admin::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Currency: ReservableCurrency<Self::AccountId> + Currency<Self::AccountId>;

		type NFTRandomness: Randomness<Self::Hash, Self::BlockNumber>;

		/// Deposit required for per byte.
		#[pallet::constant]
		type DataDepositPerByte: Get<BalanceOf<Self>>;

		type Moment: Parameter
			+ Default
			+ Scale<Self::BlockNumber, Output = Self::Moment>
			+ Copy
			+ MaxEncodedLen
			+ StaticTypeInfo
			+ MaybeSerializeDeserialize
			+ Send
			+ Into<u64>;

		type Timestamp: Time<Moment = Self::Moment>;

		type BrandCurrency: BrandTransferToken<Self::AccountId>;
	}

	// Errors
	#[pallet::error]
	pub enum Error<T> {
		NoNFT,
		NoCollection,
		NotOwner,
		DuplicateNFT,
		DuplicateCollection,
		TransferToSelf,
		NotSelling,
		NFTOnSale,
		BurntNFT,
		TokenInCollection,
		PriceNotMatch,
		BrandNotExist,
		NotPayExactAmount,
		Invalid
	}

	// Events
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Created { nft: [u8; 16], owner: T::AccountId },
		CreatedCollection { collection: [u8; 16], owner: T::AccountId },
		Edited { nft: [u8; 16], owner: T::AccountId },
		EditedCollection { collection: [u8; 16], owner: T::AccountId },
		PriceSet { nft: [u8; 16], price: Option<BalanceOf<T>> },
		NFTOnSale { nft: [u8; 16], price: Option<BalanceOf<T>> },
		BurntNFT { nft: [u8; 16] },
		DestroyCollection { collection: [u8; 16] },
		Bought { seller: T::AccountId, buyer: T::AccountId, nft: [u8; 16], price: BalanceOf<T> },
		Transferred { from: T::AccountId, to: T::AccountId, nft: [u8; 16] },
		Paid { nft_id: [u8; 16] },
		ReturnedOverdueNFT { nft_id: [u8; 16] },
		RenewNFT { nft: [u8; 16], price: BalanceOf<T> },
	}

	#[pallet::storage]
	#[pallet::getter(fn collection_by_id)]
	pub(super) type CollectionById<T: Config> = StorageMap<_, Twox64Concat, [u8; 16], NFTCollection<T::AccountId, BalanceOf<T>, T::Moment>>;

	#[pallet::storage]
	#[pallet::getter(fn token_by_id)]
	pub(super) type TokenById<T: Config> = StorageMap<_, Twox64Concat, [u8; 16], NonFungibleToken<T::AccountId, BalanceOf<T>, T::Moment>>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn mint_nft(
			origin: OriginFor<T>,
			title: Vec<u8>,
			description: Option<Vec<u8>>,
			media: Vec<u8>,
			collection_id: [u8; 16],
			price: BalanceOf<T>,
			expire: u8,
			renew_fee: BalanceOf<T>,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			let brand = pallet_brand_admin::Pallet::<T>::brand_by_id(&sender);
			ensure!(brand != None, Error::<T>::BrandNotExist);

			let nft_id = Self::gen_id();

			let data_compressed:u8 = u8::try_from(title.len()).unwrap().saturating_add(u8::try_from(description.len()).unwrap()).saturating_add(u8::try_from(media.len()).unwrap()).saturating_add(u8::try_from(collection_id.len()).unwrap()).saturating_add(32);
			let data_deposit = T::DataDepositPerByte::get().saturating_mul(data_compressed.into());
			
			T::Currency::reserve(&sender, data_deposit.clone())?;
			
			let nft = NonFungibleToken::<T::AccountId, BalanceOf<T>, T::Moment> { 
				id: nft_id.clone(),
				title,
				description,
				media,
				creator: sender.clone(),
				owner: sender.clone(),
				collection_id,
				deposit: data_deposit,
				price,
				expire,
				created_at: T::Timestamp::now(),
				renew_time: T::Timestamp::now(),
				renew_fee,
			};
			
			ensure!(!TokenById::<T>::contains_key(&nft_id), Error::<T>::DuplicateNFT);

			TokenById::<T>::insert(nft_id, nft);

			// Deposit our event.
			Self::deposit_event(Event::Created { nft: nft_id, owner: sender });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn create_collection(
			origin: OriginFor<T>,
			title: Vec<u8>,
			description: Option<Vec<u8>>,
			expire: u8
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			let brand = pallet_brand_admin::Pallet::<T>::brand_by_id(&sender);
			ensure!(brand != None, Error::<T>::BrandNotExist);

			let collection_id = Self::gen_id();

			let data_compressed:u32 = u32::try_from(title.len()).unwrap().saturating_add(u32::try_from(description.len()).unwrap()).saturating_add(16);
			let data_deposit = T::DataDepositPerByte::get().saturating_mul(data_compressed.into());

			T::Currency::reserve(&sender, data_deposit.clone())?;
			
			let collection = NFTCollection::<T::AccountId, BalanceOf<T>, T::Moment> { 
				title,
				description,
				creator: sender.clone(),
				deposit: data_deposit,
				expire,
				created_at: T::Timestamp::now()
			};
			
			ensure!(!CollectionById::<T>::contains_key(&collection_id), Error::<T>::DuplicateCollection);

			CollectionById::<T>::insert(collection_id, collection);

			// Deposit our event.
			Self::deposit_event(Event::CreatedCollection { collection: collection_id, owner: sender });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn destroy_collection(
			origin: OriginFor<T>,
			collection_id: [u8; 16],
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			let brand = pallet_brand_admin::Pallet::<T>::brand_by_id(&sender);
			ensure!(brand != None, Error::<T>::BrandNotExist);

			ensure!(CollectionById::<T>::contains_key(&collection_id), Error::<T>::NoCollection);
			let collection = CollectionById::<T>::get(collection_id.clone()).unwrap();
			ensure!(collection.creator == sender.clone(), Error::<T>::NotOwner);

			let mut check = 0;
			for nft in TokenById::<T>::iter_values() {
				if nft.collection_id == collection_id {
					check += 1;
					break;
				}
			}

			ensure!(check == 0, Error::<T>::TokenInCollection);

			T::Currency::unreserve(&sender, collection.deposit);

			CollectionById::<T>::remove(collection_id.clone());

			// Deposit our event.
			Self::deposit_event(Event::DestroyCollection { collection: collection_id });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn edit_nft(
			origin: OriginFor<T>,
			nft_id: [u8; 16],
			title: Vec<u8>,
			description: Option<Vec<u8>>,
			media: Vec<u8>,
			collection_id: [u8; 16]
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			let brand = pallet_brand_admin::Pallet::<T>::brand_by_id(&sender);
			ensure!(brand != None, Error::<T>::BrandNotExist);

			let mut nft = TokenById::<T>::get(&nft_id).ok_or(Error::<T>::NoNFT)?;
			ensure!(nft.owner == sender.clone(), Error::<T>::NotOwner);

			T::Currency::unreserve(&sender, nft.deposit);

			let data_compressed:u32 = u32::try_from(title.len()).unwrap().saturating_add(u32::try_from(description.len()).unwrap()).saturating_add(u32::try_from(media.len()).unwrap()).saturating_add(u32::try_from(collection_id.len()).unwrap()).saturating_add(32);
			let data_deposit = T::DataDepositPerByte::get().saturating_mul(data_compressed.into());

			T::Currency::reserve(&sender, data_deposit.clone())?;

			nft.title = title;
			nft.description = description;
			nft.media = media;
			nft.collection_id = collection_id;
			nft.deposit = data_deposit;

			TokenById::<T>::insert(nft_id, nft);

			// Deposit our event.
			Self::deposit_event(Event::Edited { nft: nft_id, owner: sender });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn edit_collection(
			origin: OriginFor<T>,
			collection_id: [u8; 16],
			title: Option<Vec<u8>>,
			description: Option<Vec<u8>>,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			let brand = pallet_brand_admin::Pallet::<T>::brand_by_id(&sender);
			ensure!(brand != None, Error::<T>::BrandNotExist);

			let mut collection = CollectionById::<T>::get(&collection_id).ok_or(Error::<T>::NoCollection)?;
			ensure!(collection.creator == sender.clone(), Error::<T>::NotOwner);

			T::Currency::unreserve(&sender, collection.deposit);

			let data_compressed:u32 = u32::try_from(title.len()).unwrap().saturating_add(u32::try_from(description.len()).unwrap()).saturating_add(16);
			let data_deposit = T::DataDepositPerByte::get().saturating_mul(data_compressed.into());

			T::Currency::reserve(&sender, data_deposit.clone())?;
			
			collection.title = title.unwrap();
			collection.description = description;
			collection.deposit = data_deposit;
			
			CollectionById::<T>::insert(collection_id, collection);

			// Deposit our event.
			Self::deposit_event(Event::EditedCollection { collection: collection_id, owner: sender });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn buy_nft(
			origin: OriginFor<T>,
			nft_id: [u8; 16]
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let buyer = ensure_signed(origin)?;

			Self::do_transfer(nft_id, buyer)?;

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn redeem_nft(
			origin: OriginFor<T>,
			nft_id: [u8; 16],
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut nft = TokenById::<T>::get(&nft_id).ok_or(Error::<T>::NoNFT)?;
			let from = nft.owner.clone();

			ensure!(nft.price != Self::u8_to_balance(0u8), Error::<T>::NotSelling);
			ensure!(from != sender.clone(), Error::<T>::TransferToSelf);
			ensure!(nft.creator == nft.owner, Error::<T>::Invalid);

			T::BrandCurrency::do_transfer(
				sender.clone(),
				nft.owner.clone(),
				nft.owner.clone(),
				Self::balance_to_u32(nft.price)
			)?;

			let old_owner = from;
			let new_owner = sender;

			// Deposit sold event
			Self::deposit_event(Event::Bought {
				seller: old_owner.clone(),
				buyer: new_owner.clone(),
				nft: nft_id,
				price: nft.price.clone(),
			});

			let default_price:BalanceOf<T> = 0u32.into();
			nft.owner = new_owner.clone();
			nft.price = default_price.clone();
			
			nft.deposit = default_price.clone();
			// Write updates to storage
			TokenById::<T>::insert(&nft_id, nft);

			Self::deposit_event(Event::Transferred { from: old_owner, to: new_owner.clone(), nft: nft_id });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn burn_nft(
			origin: OriginFor<T>,
			nft_id: [u8; 16]
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			let brand = pallet_brand_admin::Pallet::<T>::brand_by_id(&sender);
			ensure!(brand != None, Error::<T>::BrandNotExist);

			let nft = TokenById::<T>::get(&nft_id).ok_or(Error::<T>::NoNFT)?;
			ensure!(nft.owner == sender.clone(), Error::<T>::NotOwner);

			T::Currency::unreserve(&sender, nft.deposit);

			// Set the price in storage
			TokenById::<T>::remove(&nft_id);

			Self::deposit_event(Event::BurntNFT { nft: nft_id });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn set_nft_price(
			origin: OriginFor<T>,
			nft_id: [u8; 16],
			new_price: BalanceOf<T>,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			let mut nft = TokenById::<T>::get(&nft_id).ok_or(Error::<T>::NoNFT)?;
			ensure!(nft.owner == sender, Error::<T>::NotOwner);
			ensure!(nft.price != Self::u8_to_balance(0u8), Error::<T>::NotSelling);

			nft.price = new_price.clone();
			TokenById::<T>::insert(&nft_id, nft);

			// Deposit a "PriceSet" event.
			Self::deposit_event(Event::PriceSet { nft: nft_id, price: Some(new_price) });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn renew_nft(
			origin: OriginFor<T>,
			expire: u8,
			paid: BalanceOf<T>,
			nft_id: [u8; 16],
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;
			let mut nft = TokenById::<T>::get(&nft_id).ok_or(Error::<T>::NoNFT)?;
			ensure!(nft.renew_fee == paid, Error::<T>::NotPayExactAmount);
			ensure!(nft.owner == sender, Error::<T>::NotOwner);

			T::Currency::transfer(&sender, &nft.creator, paid.clone(), ExistenceRequirement::KeepAlive)?;

			nft.expire = expire;
			nft.renew_time = T::Timestamp::now();
			TokenById::<T>::insert(&nft_id, nft);

			// Deposit a "PriceSet" event.
			Self::deposit_event(Event::RenewNFT { nft: nft_id, price: paid });

			Ok(())
		}
	}

	//** Our helper functions.**//
	impl<T: Config> Pallet<T> {
		pub fn gen_id() -> [u8; 16] {
			// Create randomness
			let random = T::NFTRandomness::random(&b"id"[..]).0;

			// Create randomness payload. Multiple kitties can be generated in the same block,
			// retaining uniqueness.
			let unique_payload = (
				random,
				frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
				frame_system::Pallet::<T>::block_number(),
			);

			// Turns into a byte array
			let encoded_payload = unique_payload.encode();
			let hash = blake2_128(&encoded_payload);

			hash
		}

		#[require_transactional]
		pub fn do_transfer(
			nft_id: [u8; 16],
			to: T::AccountId,
		) -> DispatchResult {
			let mut nft = TokenById::<T>::get(&nft_id).ok_or(Error::<T>::NoNFT)?;
			let from = nft.owner;

			ensure!(nft.price != Self::u8_to_balance(0u8), Error::<T>::NotSelling);
			ensure!(from != to.clone(), Error::<T>::TransferToSelf);
			
			let old_owner = from;
			let new_owner = to;

			// Transfer the amount from buyer to seller
			T::Currency::transfer(&new_owner, &old_owner, nft.price.clone(), ExistenceRequirement::KeepAlive)?;
			// Deposit sold event
			Self::deposit_event(Event::Bought {
				seller: old_owner.clone(),
				buyer: new_owner.clone(),
				nft: nft_id,
				price: nft.price.clone(),
			});

			let default_price:BalanceOf<T> = 0u32.into();
			nft.owner = new_owner.clone();
			nft.price = default_price.clone();
			
			T::Currency::unreserve(&old_owner, nft.deposit.saturating_add(nft.deposit));
			
			nft.deposit = default_price.clone();
			// Write updates to storage
			TokenById::<T>::insert(&nft_id, nft);

			Self::deposit_event(Event::Transferred { from: old_owner, to: new_owner.clone(), nft: nft_id });

			Ok(())
		}

		pub fn u8_to_balance(input: u8) -> BalanceOf<T> {
			input.into()
		}

		pub fn u32_to_balance(input: u32) -> BalanceOf<T> {
			input.into()
		}

		pub fn balance_to_u8(input: BalanceOf<T>) -> u8 {
			TryInto::<u8>::try_into(input).ok().unwrap()
		}

		pub fn balance_to_u32(input: BalanceOf<T>) -> u32 {
			TryInto::<u32>::try_into(input).ok().unwrap()
		}

		pub fn get_nfts() -> Vec<NonFungibleToken<T::AccountId, BalanceOf<T>, T::Moment>> {
			TokenById::<T>::iter_values().collect()
		}

		pub fn check_expire_nft() -> DispatchResult {
			let now = T::Timestamp::now();
			let nfts = Self::get_nfts();

			for mut nft in nfts.into_iter() {
				let now_u64 = now.saturated_into::<u64>();
				let order_time_u64 = nft.created_at.saturated_into::<u64>();
				let diff = now_u64.saturating_sub(order_time_u64);
				let expire_months = nft.expire;
				// check if over 30*expire_months days + 7days
				if diff > 2592000.saturating_mul(expire_months.into()).saturating_add(604800) && nft.owner != nft.creator {
					// return nft to brand
					let creator = nft.creator.clone();
					nft.owner = creator;
					TokenById::<T>::insert(nft.id, &nft);
					Self::deposit_event(Event::ReturnedOverdueNFT { nft_id: nft.id });
				}
			}

			Ok(())
		}
	}
}