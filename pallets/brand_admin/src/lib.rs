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
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_std::vec::Vec;
	
	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Brand {
		pub name: Vec<u8>,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// The pallet's runtime storage items.
	// https://docs.substrate.io/v3/runtime/storage
	#[pallet::storage]
	#[pallet::getter(fn brand_by_id)]
	pub type BrandById<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, Brand>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NewBrandCreated(T::AccountId, Brand),
        BrandRemoved(T::AccountId, Brand)
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
        BrandNameExisted,
        ThisUserNotCreatedBrandBefore,
        BrandNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn create_brand(origin: OriginFor<T>, name: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			
			let mut check = 0;
			for brand in BrandById::<T>::iter_values() {
				if brand.name == name {
					check += 1;
					break;
				}
			}
            
			ensure!(check == 0, Error::<T>::BrandNameExisted);
            let new_brand = Brand { 
                name
            };

            BrandById::<T>::insert(&who, &new_brand);

			Self::deposit_event(Event::NewBrandCreated(who, new_brand));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn remove_brand(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

            let brand = BrandById::<T>::get(&who).ok_or(Error::<T>::ThisUserNotCreatedBrandBefore)?;

            BrandById::<T>::remove(&who);

			Self::deposit_event(Event::BrandRemoved(who, brand));
			Ok(())
		}
	}
}
