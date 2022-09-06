#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

// #[cfg(test)]
// mod mock;

// #[cfg(test)]
// mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::{DispatchResult, *},
		sp_runtime::traits::Saturating,
		sp_runtime::traits::Scale,
		sp_runtime::SaturatedConversion,
		traits::{Currency, ReservableCurrency, Time},
	};
	use frame_system::pallet_prelude::*;
	use scale_info::prelude::vec;
	use scale_info::{StaticTypeInfo, TypeInfo};
	use sp_std::vec::Vec;

	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_brand_admin::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Currency: ReservableCurrency<Self::AccountId> + Currency<Self::AccountId>;

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
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct BrandToken {
		pub symbol: Vec<u8>,
		pub amount: u32,
		pub staked: u32,
		pub default_lifetime: u8,
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Amount<Time> {
		pub amount: u32,
		pub issued_date: Time,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// The pallet's runtime storage items.
	// https://docs.substrate.io/v3/runtime/storage
	#[pallet::storage]
	#[pallet::getter(fn brand_token_by_id)]
	pub type BrandTokenById<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, BrandToken>;

	#[pallet::storage]
	#[pallet::getter(fn utxo)]
	pub type UTXO<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		T::AccountId,
		Vec<Amount<T::Moment>>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BrandTokenCreated { brand_id: T::AccountId },
		Mint { amount: u32 },
		Burn { amount: u32 },
		Transferred { amount: u32, from: T::AccountId, to: T::AccountId },
		Earned { amount: u32, issued_date: T::Moment },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		BrandNotExist,
		AlreadyCreatedToken,
		BrandTokenNotFound,
		InsufficentAmount,
		InsufficentBalance,
		InvalidAmount,
		NotSupportedYet,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn create_new_token(
			origin: OriginFor<T>,
			symbol: Vec<u8>,
			staked_amount: u32,
			default_lifetime: u8,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let brand = pallet_brand_admin::Pallet::<T>::brand_by_id(&sender);
			ensure!(brand != None, Error::<T>::BrandNotExist);
			ensure!(!BrandTokenById::<T>::contains_key(&sender), Error::<T>::AlreadyCreatedToken);

			T::Currency::reserve(&sender, Self::u32_to_balance(staked_amount.clone()))?;

			let new_token = BrandToken {
				symbol,
				amount: staked_amount,
				staked: staked_amount,
				default_lifetime,
			};

			BrandTokenById::<T>::insert(&sender, new_token.clone());

			Self::deposit_event(Event::BrandTokenCreated { brand_id: sender });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn mint(origin: OriginFor<T>, amount: u32) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut brand_token =
				BrandTokenById::<T>::get(&sender).ok_or(Error::<T>::BrandTokenNotFound)?;
			brand_token.staked = brand_token.staked + amount;
			brand_token.amount = brand_token.amount + amount;

			T::Currency::reserve(&sender, Self::u32_to_balance(amount.clone()))?;

			BrandTokenById::<T>::insert(&sender, brand_token);

			Self::deposit_event(Event::Mint { amount });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn burn(origin: OriginFor<T>, amount: u32) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut brand_token =
				BrandTokenById::<T>::get(&sender).ok_or(Error::<T>::BrandTokenNotFound)?;
			ensure!(amount <= brand_token.amount, Error::<T>::InsufficentAmount);

			brand_token.staked = brand_token.staked - amount;
			brand_token.amount = brand_token.amount - amount;

			T::Currency::unreserve(&sender, Self::u32_to_balance(amount.clone()));

			BrandTokenById::<T>::insert(&sender, brand_token);

			Self::deposit_event(Event::Burn { amount });

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn earn(origin: OriginFor<T>, amount: u32, brand_id: T::AccountId) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let mut utxo = UTXO::<T>::get(&brand_id, &sender).unwrap_or(vec![]);

			let mut brand_token = BrandTokenById::<T>::get(&brand_id).unwrap();
			brand_token.amount = brand_token.amount - amount;
			BrandTokenById::<T>::insert(&brand_id, brand_token);

			let new_amount = Amount { amount, issued_date: T::Timestamp::now() };
			utxo.push(new_amount);

			UTXO::<T>::insert(brand_id, &sender, utxo);

			Self::deposit_event(Event::Earned { amount, issued_date: T::Timestamp::now() });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn u32_to_balance(input: u32) -> BalanceOf<T> {
			input.into()
		}

		pub fn cal_sum_balance(brand_id: &T::AccountId, user_id: &T::AccountId) -> u32 {
			let utxo = UTXO::<T>::get(brand_id, user_id).ok_or(Error::<T>::InsufficentBalance);
			let sum = utxo.unwrap_or(vec![]).iter().map(|x| x.amount).sum();
			sum
		}
	}

	pub trait BrandTransferToken<AccountId> {
		fn do_transfer(
			from: AccountId,
			to: AccountId,
			brand_id: AccountId,
			amount: u32,
		) -> DispatchResult;
	}

	impl<T: Config> BrandTransferToken<T::AccountId> for Pallet<T> {
		fn do_transfer(
			from: T::AccountId,
			to: T::AccountId,
			brand_id: T::AccountId,
			amount: u32,
		) -> DispatchResult {
			let brand_token = BrandTokenById::<T>::get(&to);
			let mut utxo = UTXO::<T>::get(&brand_id, &from)
				.ok_or(Error::<T>::InsufficentBalance)
				.unwrap_or(vec![]);
			let sum = Self::cal_sum_balance(&brand_id, &from);
			ensure!(amount <= sum, Error::<T>::InsufficentBalance);

			if brand_token == None {
				// pending
				ensure!(1 == 0, Error::<T>::NotSupportedYet);
			} else {
				let mut token = brand_token.clone().unwrap();
				let now = T::Timestamp::now();
				let now_u64 = now.saturated_into::<u64>();

				let mut tmp_amount = amount;
				for item in utxo.iter_mut() {
					// over 30days * default_lifetime (months) -> remove
					if now_u64.saturating_sub(item.issued_date.saturated_into::<u64>())
						>= 2592000.saturating_mul(token.default_lifetime.into())
					{
						// return expired utxo to brand
						token.amount = token.clone().amount + item.amount;
						BrandTokenById::<T>::insert(&brand_id, token.clone());
						
						item.amount = 0;
						continue;
					}
					if item.amount >= tmp_amount {
						item.amount = item.amount - tmp_amount;
						tmp_amount = 0;
					} else if item.amount < tmp_amount {
						tmp_amount = tmp_amount - item.amount;
						item.amount = 0;
					}
				}
				token.amount = token.amount + amount;
				BrandTokenById::<T>::insert(&brand_id, token.clone());

				utxo.retain(|x| x.amount != 0);
				UTXO::<T>::insert(brand_id, &from, utxo);
			}

			Self::deposit_event(Event::Transferred { amount, from, to });

			Ok(())
		}
	}
}
