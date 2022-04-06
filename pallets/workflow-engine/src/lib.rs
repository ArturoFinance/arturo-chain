#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;

	use frame_support::{ 
		traits::{Currency, Get},
		sp_runtime::{
			ArithmeticError, DispatchError
		}
	};
	use sp_std::vec::Vec;

	pub type WorkflowId = u32;
	pub type WorkflowStepId = u8;
	pub type AssetId = u32;
	pub type NetworkId = u32;
	pub type ProtocolId = u32;
	pub type Percentage = u32;
	pub type PoolId = u32;
	pub type TokenAllocation = (AssetId, ProtocolId, Percentage);
	pub type TokenAllocationList = Vec<TokenAllocation>;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: Currency<Self::AccountId>;
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum Signal {
		TestBalanceUp,
		TestBalanceDown,
		LPTokenTresholdReached(LPTokenCharachteristics),
		TokenPriceTresholdReached(Percentage)
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum Action {
		MakeTransfer,
		LiquidityMiningAllocation(TokenAllocationList),
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct WorkflowStep {
		id: WorkflowStepId,
		signal: Signal,
		action: Action,
		is_active: bool,
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct WorkflowStepInput {
		signal: Signal,
		action: Action,
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct LPTokenCharachteristics {
		network_id: NetworkId,
		protocol_id: ProtocolId,
		pool_id: PoolId,
		percentage: Percentage
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct Workflow<AccountId> {
		id: WorkflowId,
		owner: AccountId,
		workflow_steps: Vec<WorkflowStep>,
		is_active: bool,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn workflow_ids)]
	pub type WorkflowIds<T: Config> = StorageValue<_, WorkflowId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn workflows)]
	pub(super) type Workflows<T: Config> =
		StorageMap<_, Blake2_128, WorkflowId, Workflow<T::AccountId>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn account_check)]
	pub type AccountCheck<T: Config> = StorageMap<_, Blake2_128, u8, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn payment_check)]
	pub type PaymentCheck<T: Config> = StorageValue<_, u8, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		WorkflowStored(T::AccountId, u32),
		TransferEffective(T::AccountId, T::AccountId, BalanceOf<T>),
		AccountCheckStored(u8),
		WorkflowUpdated(WorkflowId),
		WorkflowPaused(WorkflowId),
	}

	#[pallet::error]
	pub enum Error<T> {
		NoneValue,
		StorageOverflow,
		/// The workflow is not  not found in the database
		WorkflowNotFound,
		/// the functions's caller is no the workflow owner
		NotWorkflowOwner
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create_workflow(
			origin: OriginFor<T>,
			workflow_step_inputs: Vec<WorkflowStepInput>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let workflow_id = Self::generate_workflow_id()?;
			let workflow_steps = Self::identify_workflow_steps(workflow_step_inputs)?;

			let workflow: Workflow<T::AccountId> = Workflow {
				id: workflow_id,
				owner: who.clone(),
				workflow_steps,
				is_active: true,
			};

			<Workflows<T>>::insert(workflow_id, workflow);

			Self::deposit_event(Event::WorkflowStored(who, workflow_id));

			Ok(())
		}
		
		#[pallet::weight(10_000)]
		pub fn update_workflow(
			origin: OriginFor<T>,
			workflow_id: WorkflowId,
			steps: Vec<WorkflowStep>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

            Workflows::<T>::try_mutate(workflow_id, |w| -> DispatchResult {
                let workflow = w.as_mut().ok_or(Error::<T>::WorkflowNotFound)?;
                ensure!(
                    workflow.owner == who,
                    Error::<T>::NotWorkflowOwner
                );

				workflow.workflow_steps = steps;
               //TODO: for add workflow workflow.workflow_steps.push(steps);
                Ok(())
            })?;

			Self::deposit_event(Event::WorkflowUpdated(workflow_id));

			Ok(())
		}


		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn pause_workflow(
			origin: OriginFor<T>,
			workflow_id: WorkflowId
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Workflows::<T>::try_mutate(workflow_id, |w| -> DispatchResult {
                let workflow = w.as_mut().ok_or(Error::<T>::WorkflowNotFound)?;
                ensure!(
                    workflow.owner == who,
                    Error::<T>::NotWorkflowOwner
                );
				workflow.is_active = false;
                Ok(())
            })?;

			Self::deposit_event(Event::WorkflowPaused(workflow_id));

			Ok(())
		}

	}

	impl<T: Config> Pallet<T> {
		fn generate_workflow_id() -> Result<WorkflowId, DispatchError> {
			WorkflowIds::<T>::try_mutate(|next_id| -> Result<WorkflowId, DispatchError> {
				let current_id = *next_id;
				*next_id = next_id.checked_add(1u32).ok_or(ArithmeticError::Overflow)?;
				Ok(current_id)
			})
		}

		fn identify_workflow_steps(workflow_step_inputs: Vec<WorkflowStepInput>) -> Result<Vec<WorkflowStep>, DispatchError> {

			let mut steps: Vec<WorkflowStep> = Vec::new();

			for (pos, s) in workflow_step_inputs.iter().enumerate() {
				let step = WorkflowStep{
					id: pos as u8,
					signal: s.signal.clone(),
					action: s.action.clone(),
					is_active: true,
				};
				steps.push(step)
			}
			Ok(steps)
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(_n: T::BlockNumber) {
			log::info!("Hello from an offchain worker 👋");

		}
	}
}
