use std::collections::HashMap;

use near_sdk::{
    env, ext_contract, is_promise_success, json_types::U128, log, near,
    require, AccountId, PanicOnDefault, Promise,
};

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct StatusMessage {
    data: String,
    pause_status: bool,
    w_near_contract: AccountId,
    owner: AccountId,
    account_balances: HashMap<AccountId, u128>,
}

#[ext_contract(ft)]
pub trait FT {
    fn ft_transfer(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
    );
}

#[near]
impl StatusMessage {
    #[init]
    pub fn init(owner: AccountId, w_near_contract: AccountId) -> Self {
        Self {
            owner,
            data: String::from("Hello World!"),
            pause_status: false,
            w_near_contract,
            account_balances: HashMap::new(),
        }
    }

    pub fn get_pause_status(&self) -> bool {
        self.pause_status
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }

    pub fn buy_points(&mut self) {
        let account_id = env::predecessor_account_id();
        let deposit = env::attached_deposit().as_yoctonear() * 2;
        let balance = self.account_balances.get(&account_id).unwrap_or(&0);
        self.account_balances.insert(account_id, balance + deposit);
    }

    pub fn get_user_points(&self, account_id: AccountId) -> U128 {
        U128(*self.account_balances.get(&account_id).unwrap_or(&0))
    }

    pub fn withdraw_points_wnear(&mut self, amount: u128) {
        let account_id = env::predecessor_account_id();
        let balance = self.account_balances.get(&account_id).unwrap_or(&0);
        require!(*balance >= amount, "Not enough points");

        self.account_balances
            .insert(account_id.clone(), balance - amount);

        ft::ext(self.w_near_contract.clone())
            .ft_transfer(account_id.clone(), amount.into(), None)
            .then(
                Self::ext(env::current_account_id())
                    .resolve_withdraw(account_id, amount),
            );
    }

    pub fn resolve_withdraw(&mut self, account_id: AccountId, amount: u128) {
        if is_promise_success() {
            log!("Withdraw succeeded");
        } else {
            log!("Withdraw failed");
            let balance = self.account_balances.get(&account_id).unwrap_or(&0);
            self.account_balances.insert(account_id, balance + amount);
        }
    }

    pub fn get_data(&self) -> String {
        self.when_not_paused();
        self.data.clone()
    }

    pub fn pub_toggle_pause(&mut self) {
        require!(
            env::predecessor_account_id() == self.owner,
            "Only owner can call this function"
        );
        self.toggle_pause()
    }

    pub fn set_owner(&mut self, new_owner: AccountId) {
        require!(
            env::signer_account_id() == self.owner,
            "Only owner can call this function"
        );
        self.owner = new_owner;
    }
}

pub trait Pausable {
    fn toggle_pause(&mut self);
    fn pause(&mut self);
    fn unpause(&mut self);
    fn when_not_paused(&self);
}

#[near]
impl Pausable for StatusMessage {
    fn toggle_pause(&mut self) {
        if !self.pause_status {
            self.pause()
        } else {
            self.unpause()
        }
    }

    fn pause(&mut self) {
        self.pause_status = true;
        env::log_str("The system is paused")
    }

    fn unpause(&mut self) {
        self.pause_status = false;
        env::log_str("The system is unpaused")
    }

    fn when_not_paused(&self) {
        if self.pause_status {
            env::panic_str("Function is paused")
        }
    }
}
