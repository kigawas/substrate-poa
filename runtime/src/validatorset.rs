use rstd::prelude::*;
use session;
use sr_primitives::traits::{Convert, Member, OpaqueKeys, Zero};
use support::{
	decl_event, decl_module, decl_storage, dispatch::Result, ensure, Parameter, StorageMap,
};
use system::{ensure_root, ensure_signed};

use crate::types::SessionIndex;

pub trait Trait: system::Trait + session::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
	trait Store for Module<T: Trait> as ValidatorSet {
		Validators get(validators): Vec<T::AccountId>;
		AddProposals get(add_proposals): map T::AccountId => bool;
		RemovalProposals get(removal_proposals): map T::AccountId  => bool;
		AddVotes get(add_votes): map T::AccountId => Vec<T::AccountId>;
		RemovalVotes get(removal_votes): map T::AccountId => Vec<T::AccountId>;
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as system::Trait>::AccountId,
	{
		// New validator proposed. First argument is the AccountId of proposer.
		ValidatorProposed(AccountId, AccountId),

		// Validator removal proposed. First argument is the AccountId of proposer.
		ValidatorRemovalProposed(AccountId, AccountId),

		// New validator added.
		ValidatorAdded(AccountId),

		// Validator removed.
		ValidatorRemoved(AccountId),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		/// Propose a new validator to be added.
		///
		/// Can only be called by an existing validator.
		pub fn propose_validator(origin, account_id: T::AccountId) -> Result {
			let who = ensure_signed(origin)?;
			ensure!(Self::is_validator(who.clone()), "Access Denied!");
			ensure!(!Self::is_validator(account_id.clone()), "Already a validator.");

			if <AddProposals<T>>::exists(account_id.clone()) {
				let votes = <AddVotes<T>>::get(account_id.clone());
				let v = votes.into_iter().find(|x| x == &who);
				ensure!(v == None, "You have already proposed this validator.");
			} else {
				<AddProposals<T>>::insert(account_id.clone(), true);
			}

			<AddVotes<T>>::mutate(account_id.clone(), |vote_list| {
				vote_list.push(who.clone());
			});

			Self::deposit_event(RawEvent::ValidatorProposed(who, account_id));
			Ok(())
		}

		/// Verifies if all existing validators have proposed the new validator
		/// and then adds the new validator.
		///
		/// New validator's session key should be set in session module before calling this.
		pub fn resolve_add_validator(origin, account_id: T::AccountId) -> Result {
			let _who = ensure_signed(origin)?;

			// ensure!(!<Validators<T>>::exists(account_id.clone()), "Already a validator.");
			ensure!(<AddProposals<T>>::exists(account_id.clone()),
				"Proposal to add this validator does not exist.");

			let votes = <AddVotes<T>>::get(account_id.clone());
			let current_count = <session::Module<T>>::validators().len() as u32;
			ensure!(votes.len() as u32 == current_count, "Not enough votes.");

			Self::add_new_authority(account_id)?;
			Ok(())
		}

		/// Add a new validator using root/sudo privileges.
		///
		/// New validator's session key should be set in session module before calling this.
		pub fn add_validator(origin, account_id: T::AccountId) -> Result {
			ensure_root(origin)?;
			// ensure!(!<Validators<T>>::exists(account_id.clone()), "Already a validator.");

			Self::add_new_authority(account_id)?;

			Ok(())
		}

		/// Propose the removal of a validator to be added.
		///
		/// Can only be called by an existing validator.
		pub fn propose_validator_removal(origin, account_id: T::AccountId) -> Result {
			let who = ensure_signed(origin)?;
			// ensure!(<Validators<T>>::exists(who.clone()), "Access Denied!");
			// ensure!(<Validators<T>>::exists(account_id.clone()), "Not a validator.");

			if <RemovalProposals<T>>::exists(account_id.clone()) {
				let votes = <RemovalVotes<T>>::get(account_id.clone());
				let v = votes.into_iter().find(|x| x == &who);
				ensure!(v == None, "You have already proposed removal of this validator.");
			} else {
				<RemovalProposals<T>>::insert(account_id.clone(), true);
			}

			<RemovalVotes<T>>::mutate(account_id.clone(), |vote_list| {
				vote_list.push(who.clone());
			});

			Self::deposit_event(RawEvent::ValidatorRemovalProposed(who, account_id));
			Ok(())
		}

		/// Verifies if all *other* validators have proposed the removal of a validator
		/// and then removes the new validator.
		pub fn resolve_remove_validator(origin, account_id: T::AccountId) -> Result {
			let _who = ensure_signed(origin)?;

			// ensure!(<Validators<T>>::exists(account_id.clone()), "Not a validator.");
			ensure!(<RemovalProposals<T>>::exists(account_id.clone()),
				"Proposal to remove this validator does not exist.");

			let votes = <RemovalVotes<T>>::get(account_id.clone());
			let current_count = <session::Module<T>>::validators().len() as u32;

			// To avoid iterating over two vecs to check if every other validator has voted,
			// we are simply comparing the length.
			// This is still safe enough because you cannot vote twice.
			ensure!(votes.len() as u32 == current_count - 1, "Not enough votes.");

			Self::remove_authority(account_id)?;
			Ok(())
		}

		/// Remove a validator using root/sudo privileges.
		pub fn remove_validator(origin, account_id: T::AccountId) -> Result {
			ensure_root(origin)?;
			// ensure!(<Validators<T>>::exists(account_id.clone()), "Not a validator.");

			Self::remove_authority(account_id)?;

			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	fn is_validator(account_id: T::AccountId) -> bool {
		let validators = <Validators<T>>::get();
		validators.contains(&account_id)
	}

	fn add_new_authority(account_id: T::AccountId) -> Result {
		ensure!(
			!Self::is_validator(account_id.clone()),
			"already a validator"
		);

		<Validators<T>>::mutate(|vs| {
			vs.push(account_id.clone());
		});

		// Rotate session for new set of validators to take effect.
		// <session::Module<T>>::rotate_session();

		// Self::deposit_event(RawEvent::ValidatorAdded(account_id, session_key));
		Ok(())
	}

	// Removes an authority
	fn remove_authority(account_id: T::AccountId) -> Result {
		// Find and remove validator from the current list.
		<Validators<T>>::mutate(|vs| {
			vs.retain(|v| *v != account_id.clone());
		});

		// Rotate session for new set of validators to take effect.
		// <session::Module<T>>::rotate_session();
		// <Validators<T>>::remove(account_id.clone());

		// Removing the proposals and votes so that it can be added again.
		// Should they be preserved or archived in any way?
		<AddProposals<T>>::remove(account_id.clone());
		<RemovalProposals<T>>::remove(account_id.clone());
		<AddVotes<T>>::remove(account_id.clone());
		<RemovalVotes<T>>::remove(account_id.clone());

		// Self::deposit_event(RawEvent::ValidatorRemoved(account_id, session_key));
		Ok(())
	}

	fn next_validators() -> Option<Vec<T::AccountId>> {
		Some(Self::authorities())
	}

	pub fn authorities() -> Vec<T::AccountId> {
		<Validators<T>>::get()
	}
}

impl<T: Trait> session::OnSessionEnding<T::AccountId> for Module<T> {
	fn on_session_ending(
		_ending: SessionIndex,
		_start_session: SessionIndex,
	) -> Option<Vec<T::AccountId>> {
		Self::next_validators()
	}
}
