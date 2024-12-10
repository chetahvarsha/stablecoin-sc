#![no_std]

numbat_wasm::imports!();

pub mod user_deposit;
use user_deposit::*;

#[numbat_wasm_derive::contract]
pub trait LockRewards {
    #[init]
    fn init(
        &self,
        stablecoin_token_id: TokenIdentifier,
        percentage_reward_per_block: Self::BigUint,
    ) -> SCResult<()> {
        require!(
            stablecoin_token_id.is_valid_dcdt_identifier(),
            "invalid stablecoin token id"
        );

        self.try_set_percentage_rewards_per_block(&percentage_reward_per_block)
    }

    // endpoints - owner-only

    #[endpoint(setPercentageRewardPerBlock)]
    fn set_percentage_reward_per_block(
        &self,
        percentage_reward_per_block: Self::BigUint,
    ) -> SCResult<()> {
        only_owner!(self, "only owner may call this function");

        let old_percentage = self.percentage_reward_per_block().get();
        self.try_set_percentage_rewards_per_block(&percentage_reward_per_block)?;

        let current_block_nonce = self.blockchain().get_block_nonce();
        for address in self.user_deposits().keys() {
            self.user_deposits()
                .entry(address)
                .and_modify(|user_deposit| {
                    user_deposit.accummulate_rewards(current_block_nonce, &old_percentage);
                });
        }

        Ok(())
    }

    // endpoints

    #[payable("*")]
    #[endpoint]
    fn deposit(
        &self,
        #[payment_token] token_id: TokenIdentifier,
        #[payment] amount: Self::BigUint,
    ) -> SCResult<()> {
        require!(
            token_id == self.stablecoin_token_id().get(),
            "Wrong payment token"
        );
        require!(amount > 0, "Must deposit more than 0");

        let caller = self.blockchain().get_caller();
        let current_block_nonce = self.blockchain().get_block_nonce();
        let percentage_reward_per_block = self.percentage_reward_per_block().get();

        self.user_deposits()
            .entry(caller)
            .or_default()
            .update(|user_deposit| {
                user_deposit.accummulate_rewards(current_block_nonce, &percentage_reward_per_block);
                user_deposit.amount += amount;
            });

        Ok(())
    }

    /// optional amount to withdraw. Defaults to max possible.
    #[endpoint]
    fn withdraw(&self, #[var_args] opt_amount: OptionalArg<Self::BigUint>) -> SCResult<()> {
        let caller = self.blockchain().get_caller();
        let mut user_deposit = self.get_user_deposit_or_default(&caller);
        let amount = opt_amount
            .into_option()
            .unwrap_or_else(|| user_deposit.amount.clone());

        require!(amount > 0, "Must withdraw more than 0");
        require!(
            amount <= user_deposit.amount,
            "Cannot withdraw more than deposited amount"
        );

        self.send_stablecoins(&caller, &amount);

        let current_block_nonce = self.blockchain().get_block_nonce();
        let percentage_reward_per_block = self.percentage_reward_per_block().get();
        user_deposit.accummulate_rewards(current_block_nonce, &percentage_reward_per_block);
        user_deposit.amount -= amount;

        self.update_user_deposit_or_remove_if_cleared(caller, user_deposit);

        Ok(())
    }

    #[endpoint(claimRewards)]
    fn claim_rewards(&self) -> SCResult<()> {
        let caller = self.blockchain().get_caller();
        let current_block_nonce = self.blockchain().get_block_nonce();
        let percentage_reward_per_block = self.percentage_reward_per_block().get();
        let mut user_deposit = self.get_user_deposit_or_default(&caller);

        user_deposit.accummulate_rewards(current_block_nonce, &percentage_reward_per_block);

        self.try_mint_stablecoins(&user_deposit.cummulated_rewards)?;
        self.send_stablecoins(&caller, &user_deposit.cummulated_rewards);

        user_deposit.cummulated_rewards = Self::BigUint::zero();
        self.update_user_deposit_or_remove_if_cleared(caller, user_deposit);

        Ok(())
    }

    // private

    fn require_local_mint_role_set(&self) -> SCResult<()> {
        let token_id = self.stablecoin_token_id().get();
        let roles = self.blockchain().get_dcdt_local_roles(&token_id);
        require!(
            roles.contains(&DcdtLocalRole::Mint),
            "Local Mint role not set"
        );

        Ok(())
    }

    fn try_set_percentage_rewards_per_block(
        &self,
        percentage_reward_per_block: &Self::BigUint,
    ) -> SCResult<()> {
        require!(
            *percentage_reward_per_block > 0 && *percentage_reward_per_block <= BASE_PRECISION,
            "Invalid percentage"
        );

        self.percentage_reward_per_block()
            .set(percentage_reward_per_block);

        Ok(())
    }

    fn try_mint_stablecoins(&self, amount: &Self::BigUint) -> SCResult<()> {
        self.require_local_mint_role_set()?;

        let token_id = self.stablecoin_token_id().get();
        self.send().dcdt_local_mint(&token_id, amount);

        Ok(())
    }

    fn send_stablecoins(&self, to: &Address, amount: &Self::BigUint) {
        if amount > &0 {
            let token_id = self.stablecoin_token_id().get();
            self.send().direct(to, &token_id, amount, &[]);
        }
    }

    fn get_user_deposit_or_default(&self, address: &Address) -> UserDeposit<Self::BigUint> {
        self.user_deposits().get(address).unwrap_or_default()
    }

    fn update_user_deposit_or_remove_if_cleared(
        &self,
        address: Address,
        user_deposit: UserDeposit<Self::BigUint>,
    ) {
        if user_deposit.amount > 0 || user_deposit.cummulated_rewards > 0 {
            self.user_deposits().insert(address, user_deposit);
        } else {
            self.user_deposits().remove(&address);
        }
    }

    // storage

    #[storage_mapper("stablecoinTokenId")]
    fn stablecoin_token_id(&self) -> SingleValueMapper<Self::Storage, TokenIdentifier>;

    #[storage_mapper("percentageRewardPerBlock")]
    fn percentage_reward_per_block(&self) -> SingleValueMapper<Self::Storage, Self::BigUint>;

    #[storage_mapper("userDeposits")]
    fn user_deposits(&self) -> MapMapper<Self::Storage, Address, UserDeposit<Self::BigUint>>;
}
