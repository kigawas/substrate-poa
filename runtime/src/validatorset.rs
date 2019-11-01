use rstd::prelude::*;
use session;
use sr_primitives::traits::{Convert, Member, OpaqueKeys, Zero};
use support::{
	decl_event, decl_module, decl_storage, dispatch::Result, ensure, Parameter, StorageMap,
};
use system::{ensure_root, ensure_signed};

pub trait Trait: system::Trait + session::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
	trait Store for Module<T: Trait> as ValidatorSet {
		Validators get(validators): map T::AccountId => T::Keys;
		AddProposals get(add_proposals): map (T::AccountId, T::Keys) => bool;
		RemovalProposals get(removal_proposals): map (T::AccountId, T::Keys) => bool;
		AddVotes get(add_votes): map (T::AccountId, T::Keys) => Vec<T::AccountId>;
		RemovalVotes get(removal_votes): map (T::AccountId, T::Keys) => Vec<T::AccountId>;
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as system::Trait>::AccountId,
		SessionKey = <T as session::Trait>::Keys,
	{
		// New validator proposed. First argument is the AccountId of proposer.
		ValidatorProposed(AccountId, AccountId, SessionKey),

		// Validator removal proposed. First argument is the AccountId of proposer.
		ValidatorRemovalProposed(AccountId, AccountId, SessionKey),

		// New validator added.
		ValidatorAdded(AccountId, SessionKey),

		// Validator removed.
		ValidatorRemoved(AccountId, SessionKey),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		/// Propose a new validator to be added.
		///
		/// Can only be called by an existing validator.
		pub fn propose_validator(origin, account_id: T::AccountId, session_key: <T as session::Trait>::Keys) -> Result {
			let who = ensure_signed(origin)?;
			ensure!(<Validators<T>>::exists(who.clone()), "Access Denied!");
			ensure!(!<Validators<T>>::exists(account_id.clone()), "Already a validator.");

			if <AddProposals<T>>::exists((account_id.clone(), session_key.clone())) {
				let votes = <AddVotes<T>>::get((account_id.clone(), session_key.clone()));
				let v = votes.into_iter().find(|x| x == &who);
				ensure!(v == None, "You have already proposed this validator.");
			} else {
				<AddProposals<T>>::insert((account_id.clone(), session_key.clone()), true);
			}

			<AddVotes<T>>::mutate((account_id.clone(), session_key.clone()), |vote_list| {
				vote_list.push(who.clone());
			});

			Self::deposit_event(RawEvent::ValidatorProposed(who, account_id, session_key));
			Ok(())
		}

		/// Verifies if all existing validators have proposed the new validator
		/// and then adds the new validator.
		///
		/// New validator's session key should be set in session module before calling this.
		pub fn resolve_add_validator(origin, account_id: T::AccountId, session_key: T::Keys) -> Result {
			let _who = ensure_signed(origin)?;

			ensure!(!<Validators<T>>::exists(account_id.clone()), "Already a validator.");
			ensure!(<AddProposals<T>>::exists((account_id.clone(), session_key.clone())),
				"Proposal to add this validator does not exist.");

			let votes = <AddVotes<T>>::get((account_id.clone(), session_key.clone()));
			let current_count = <session::Module<T>>::validators().len() as u32;
			ensure!(votes.len() as u32 == current_count, "Not enough votes.");

			Self::add_new_authority(account_id, session_key)?;
			Ok(())
		}

		/// Add a new validator using root/sudo privileges.
		///
		/// New validator's session key should be set in session module before calling this.
		pub fn add_validator(origin, account_id: T::AccountId, session_key: T::Keys) -> Result {
			ensure_root(origin)?;
			ensure!(!<Validators<T>>::exists(account_id.clone()), "Already a validator.");

			Self::add_new_authority(account_id, session_key)?;

			Ok(())
		}

		/// Propose the removal of a validator to be added.
		///
		/// Can only be called by an existing validator.
		pub fn propose_validator_removal(origin, account_id: T::AccountId, session_key: T::Keys) -> Result {
			let who = ensure_signed(origin)?;
			ensure!(<Validators<T>>::exists(who.clone()), "Access Denied!");
			ensure!(<Validators<T>>::exists(account_id.clone()), "Not a validator.");

			if <RemovalProposals<T>>::exists((account_id.clone(), session_key.clone())) {
				let votes = <RemovalVotes<T>>::get((account_id.clone(), session_key.clone()));
				let v = votes.into_iter().find(|x| x == &who);
				ensure!(v == None, "You have already proposed removal of this validator.");
			} else {
				<RemovalProposals<T>>::insert((account_id.clone(), session_key.clone()), true);
			}

			<RemovalVotes<T>>::mutate((account_id.clone(), session_key.clone()), |vote_list| {
				vote_list.push(who.clone());
			});

			Self::deposit_event(RawEvent::ValidatorRemovalProposed(who, account_id, session_key));
			Ok(())
		}

		/// Verifies if all *other* validators have proposed the removal of a validator
		/// and then removes the new validator.
		pub fn resolve_remove_validator(origin, account_id: T::AccountId, session_key: T::Keys) -> Result {
			let _who = ensure_signed(origin)?;

			ensure!(<Validators<T>>::exists(account_id.clone()), "Not a validator.");
			ensure!(<RemovalProposals<T>>::exists((account_id.clone(), session_key.clone())),
				"Proposal to remove this validator does not exist.");

			let votes = <RemovalVotes<T>>::get((account_id.clone(), session_key.clone()));
			let current_count = <session::Module<T>>::validators().len() as u32;

			// To avoid iterating over two vecs to check if every other validator has voted,
			// we are simply comparing the length.
			// This is still safe enough because you cannot vote twice.
			ensure!(votes.len() as u32 == current_count - 1, "Not enough votes.");

			Self::remove_authority(account_id, session_key)?;
			Ok(())
		}

		/// Remove a validator using root/sudo privileges.
		pub fn remove_validator(origin, account_id: T::AccountId, session_key: T::Keys) -> Result {
			ensure_root(origin);
			ensure!(<Validators<T>>::exists(account_id.clone()), "Not a validator.");

			Self::remove_authority(account_id, session_key)?;

			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	// Adds new authority in the consensus module.
	fn add_new_authority(account_id: T::AccountId, session_key: T::Keys) -> Result {
		// Add new validator in session module.
		// let mut current_validators = <session::Module<T>>::validators();
		// current_validators.push(account_id.clone());
		let who = match T::ValidatorIdOf::convert(account_id) {
			Some(val_id) => val_id,
			None => return Err("no associated validator ID for account."),
		};
		<session::Module<T>>::set_keys(&who);

		// Rotate session for new set of validators to take effect.
		<session::Module<T>>::rotate_session();
		<Validators<T>>::insert(account_id.clone(), session_key.clone());

		Self::deposit_event(RawEvent::ValidatorAdded(account_id, session_key));
		Ok(())
	}

	// Removes an authority
	fn remove_authority(account_id: T::AccountId, session_key: T::Keys) -> Result {
		// Find and remove validator from the current list.
		// let mut current_validators = <session::Module<T>>::validators();
		// for (i, v) in current_validators.clone().into_iter().enumerate() {
		// 	if v == account_id {
		// 		current_validators.swap_remove(i);
		// 	}
		// }
		// <session::Module<T>>::set_validators(&current_validators);
		let who = match T::ValidatorIdOf::convert(account_id) {
			Some(val_id) => val_id,
			None => return Err("no associated validator ID for account."),
		};
		<session::Module<T>>::take_keys(&who);

		// Rotate session for new set of validators to take effect.
		<session::Module<T>>::rotate_session();
		<Validators<T>>::remove(account_id.clone());

		// Removing the proposals and votes so that it can be added again.
		// Should they be preserved or archived in any way?
		<AddProposals<T>>::remove((account_id.clone(), session_key.clone()));
		<RemovalProposals<T>>::remove((account_id.clone(), session_key.clone()));
		<AddVotes<T>>::remove((account_id.clone(), session_key.clone()));
		<RemovalVotes<T>>::remove((account_id.clone(), session_key.clone()));

		Self::deposit_event(RawEvent::ValidatorRemoved(account_id, session_key));
		Ok(())
	}
}
