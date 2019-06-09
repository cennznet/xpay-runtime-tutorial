use support::{decl_module, decl_storage, decl_event, StorageValue, StorageMap, dispatch::Result, Parameter, ensure};
use runtime_primitives::traits::{CheckedAdd, CheckedMul, As};
use system::ensure_signed;

pub trait Trait: cennzx_spot::Trait {
	type Item: Parameter;
	type ItemId: Parameter + CheckedAdd + Default + From<u8>;
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type BalanceOf<T> = <T as generic_asset::Trait>::Balance;
pub type AssetIdOf<T> = <T as generic_asset::Trait>::AssetId;
pub type PriceOf<T> = (AssetIdOf<T>, BalanceOf<T>);

decl_storage! {
	trait Store for Module<T: Trait> as XPay {
		pub Items get(item): map T::ItemId => Option<T::Item>;
		pub ItemOwners get(item_owner): map T::ItemId => Option<T::AccountId>;
		pub ItemQuantities get(item_quantity): map T::ItemId => u32;
		pub ItemPrices get(item_price): map T::ItemId => Option<PriceOf<T>>;
		
		pub NextItemId get(next_item_id): T::ItemId;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		pub fn create_item(origin, quantity: u32, item: T::Item, price: PriceOf<T>) -> Result {
			let origin = ensure_signed(origin)?;

			let item_id = Self::next_item_id();

			// The last available id serves as the overflow mark and won't be used.
			let next_item_id = item_id.checked_add(&1.into()).ok_or_else(||"No new item id is available.")?;

			<NextItemId<T>>::put(next_item_id);

			<Items<T>>::insert(item_id.clone(), item.clone());
			<ItemOwners<T>>::insert(item_id.clone(), origin.clone());
			<ItemQuantities<T>>::insert(item_id.clone(), quantity);
			<ItemPrices<T>>::insert(item_id.clone(), price.clone());

			Self::deposit_event(RawEvent::ItemCreated(origin, item_id, quantity, item, price));

			Ok(())
		}

		pub fn add_item(origin, item_id: T::ItemId, quantity: u32) -> Result {
			let origin = ensure_signed(origin)?;

			<ItemQuantities<T>>::mutate(item_id.clone(), |q| *q = q.saturating_add(quantity));

			Self::deposit_event(RawEvent::ItemAdded(origin, item_id.clone(), Self::item_quantity(item_id)));

			Ok(())
		}

		pub fn remove_item(origin, item_id: T::ItemId, quantity: u32) -> Result {
			let origin = ensure_signed(origin)?;

			<ItemQuantities<T>>::mutate(item_id.clone(), |q| *q = q.saturating_sub(quantity));

			Self::deposit_event(RawEvent::ItemRemoved(origin, item_id.clone(), Self::item_quantity(item_id)));

			Ok(())
		}

		pub fn update_item(origin, item_id: T::ItemId, quantity: Option<u32>, price: Option<PriceOf<T>>) -> Result {
			let origin = ensure_signed(origin)?;

			ensure!(<Items<T>>::exists(item_id.clone()), "Item did not exist");

			if let Some(quantity) = quantity {
				<ItemQuantities<T>>::insert(item_id.clone(), quantity);
			}

			if let Some(price) = price {
				<ItemPrices<T>>::insert(item_id.clone(), price);
			}

			let new_quantity = Self::item_quantity(item_id.clone());
			let new_price = Self::item_price(item_id.clone()).expect("Item exists; Item price must exists; qed");
			Self::deposit_event(RawEvent::ItemUpdated(origin, item_id, new_quantity, new_price));

			Ok(())
		}

		pub fn purchase_item(origin, quantity: u32, item_id: T::ItemId, max_total_price: PriceOf<T>) -> Result {
			let origin = ensure_signed(origin)?;

			let new_quantity = Self::item_quantity(item_id.clone()).checked_sub(quantity).ok_or_else(||"Not enough quantity")?;
			let item_price = Self::item_price(item_id.clone()).ok_or_else(||"No item price")?;
			let seller = Self::item_owner(item_id.clone()).ok_or_else(||"No item owner")?;

			let total_price_amount = item_price.1.checked_mul(&As::sa(quantity as u64)).ok_or_else(||"Total price overflow")?;

			if item_price.0 == max_total_price.0 {
				// Same asset, GA transfer

				ensure!(total_price_amount < max_total_price.1, "User paying price too low");

				<generic_asset::Module<T>>::make_transfer_with_event(&item_price.0, &origin, &seller, total_price_amount)?;
			} else {
				// Different asset, CENNZX-Spot transfer

				<cennzx_spot::Module<T>>::make_asset_swap_output(
					&origin,             // buyer
					&seller,             // recipient
					&max_total_price.0,  // asset_sold
					&item_price.0,       // asset_bought
					item_price.1,       // buy_amount
					max_total_price.1,  // max_paying_amount
					<cennzx_spot::Module<T>>::fee_rate() // fee_rate
				)?;
			}

			<ItemQuantities<T>>::insert(item_id.clone(), new_quantity);

			Self::deposit_event(RawEvent::ItemSold(origin, item_id, quantity));

			Ok(())
		}
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::Item,
		<T as Trait>::ItemId,
		Price = PriceOf<T>,
	{
		/// New item created. (transactor, item_id, quantity, item, price)
		ItemCreated(AccountId, ItemId, u32, Item, Price),
		/// More items added. (transactor, item_id, new_quantity)
		ItemAdded(AccountId, ItemId, u32),
		/// Items removed. (transactor, item_id, new_quantity)
		ItemRemoved(AccountId, ItemId, u32),
		/// Item updated. (transactor, item_id, new_quantity, new_price)
		ItemUpdated(AccountId, ItemId, u32, Price),
		/// Item sold. (transactor, item_id, quantity)
		ItemSold(AccountId, ItemId, u32),
	}
);
