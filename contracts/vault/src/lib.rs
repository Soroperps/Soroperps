#![no_std]

mod shares;
mod storage;
#[cfg(test)]
mod test;

use perps_types::{PerpsError, BPS_DENOMINATOR};
use soroban_sdk::{contract, contractimpl, token, Address, Env};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

mod events {
    use soroban_sdk::{contracttype, Address, Env, Symbol};

    #[contracttype]
    #[derive(Clone)]
    pub struct DepositEvent {
        pub depositor: Address,
        pub amount: i128,
        pub shares_minted: i128,
    }

    #[contracttype]
    #[derive(Clone)]
    pub struct WithdrawEvent {
        pub depositor: Address,
        pub shares_burned: i128,
        pub amount_returned: i128,
    }

    #[contracttype]
    #[derive(Clone)]
    pub struct LiquidityLockedEvent {
        pub amount: i128,
    }

    #[contracttype]
    #[derive(Clone)]
    pub struct LiquidityUnlockedEvent {
        pub amount: i128,
    }

    #[contracttype]
    #[derive(Clone)]
    pub struct PnlSettledEvent {
        pub trader: Address,
        pub pnl: i128,
    }

    #[contracttype]
    #[derive(Clone)]
    pub struct FeeCollectedEvent {
        pub amount: i128,
    }

    pub fn emit_deposit(env: &Env, depositor: Address, amount: i128, shares_minted: i128) {
        env.events().publish(
            (Symbol::new(env, "deposit"),),
            DepositEvent {
                depositor,
                amount,
                shares_minted,
            },
        );
    }

    pub fn emit_withdraw(
        env: &Env,
        depositor: Address,
        shares_burned: i128,
        amount_returned: i128,
    ) {
        env.events().publish(
            (Symbol::new(env, "withdraw"),),
            WithdrawEvent {
                depositor,
                shares_burned,
                amount_returned,
            },
        );
    }

    pub fn emit_liquidity_locked(env: &Env, amount: i128) {
        env.events().publish(
            (Symbol::new(env, "liquidity_locked"),),
            LiquidityLockedEvent { amount },
        );
    }

    pub fn emit_liquidity_unlocked(env: &Env, amount: i128) {
        env.events().publish(
            (Symbol::new(env, "liquidity_unlocked"),),
            LiquidityUnlockedEvent { amount },
        );
    }

    pub fn emit_pnl_settled(env: &Env, trader: Address, pnl: i128) {
        env.events().publish(
            (Symbol::new(env, "pnl_settled"),),
            PnlSettledEvent { trader, pnl },
        );
    }

    pub fn emit_fee_collected(env: &Env, amount: i128) {
        env.events().publish(
            (Symbol::new(env, "fee_collected"),),
            FeeCollectedEvent { amount },
        );
    }
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct VaultContract;

#[contractimpl]
impl VaultContract {
    /// Initialize the vault. Can only be called once.
    pub fn initialize(
        env: Env,
        admin: Address,
        usdc_token: Address,
        max_utilization_bps: u32,
    ) -> Result<(), PerpsError> {
        if storage::has_admin(&env) {
            return Err(PerpsError::AlreadyInitialized);
        }
        if max_utilization_bps == 0 || max_utilization_bps > BPS_DENOMINATOR {
            return Err(PerpsError::InvalidConfig);
        }

        storage::set_admin(&env, &admin);
        storage::set_usdc_token(&env, &usdc_token);
        storage::set_max_utilization(&env, max_utilization_bps);
        storage::set_total_shares(&env, 0);
        storage::set_total_deposits(&env, 0);
        storage::set_locked_liquidity(&env, 0);
        storage::bump_instance(&env);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // LP operations
    // -----------------------------------------------------------------------

    /// LP deposits USDC, receives shares proportional to pool value.
    pub fn deposit(env: Env, depositor: Address, amount: i128) -> Result<i128, PerpsError> {
        storage::bump_instance(&env);
        depositor.require_auth();

        if amount <= 0 {
            return Err(PerpsError::InvalidAmount);
        }

        let total_shares = storage::get_total_shares(&env);
        let total_deposits = storage::get_total_deposits(&env);

        let shares_to_mint = shares::calc_shares_to_mint(amount, total_shares, total_deposits);
        if shares_to_mint <= 0 {
            return Err(PerpsError::ZeroShares);
        }

        // Transfer USDC from depositor to this contract
        let usdc = storage::get_usdc_token(&env);
        let token_client = token::TokenClient::new(&env, &usdc);
        token_client.transfer(&depositor, &env.current_contract_address(), &amount);

        // Update state
        storage::set_total_deposits(&env, total_deposits + amount);
        storage::set_total_shares(&env, total_shares + shares_to_mint);

        let current_balance = storage::get_share_balance(&env, &depositor);
        storage::set_share_balance(&env, &depositor, current_balance + shares_to_mint);

        events::emit_deposit(&env, depositor, amount, shares_to_mint);

        Ok(shares_to_mint)
    }

    /// LP burns shares, receives USDC proportional to pool value.
    pub fn withdraw(env: Env, depositor: Address, share_amount: i128) -> Result<i128, PerpsError> {
        storage::bump_instance(&env);
        depositor.require_auth();

        if share_amount <= 0 {
            return Err(PerpsError::InvalidAmount);
        }

        let current_balance = storage::get_share_balance(&env, &depositor);
        if share_amount > current_balance {
            return Err(PerpsError::InsufficientBalance);
        }

        let total_shares = storage::get_total_shares(&env);
        let total_deposits = storage::get_total_deposits(&env);
        let locked = storage::get_locked_liquidity(&env);

        let usdc_to_return =
            shares::calc_withdrawal_amount(share_amount, total_shares, total_deposits);
        if usdc_to_return <= 0 {
            return Err(PerpsError::ZeroShares);
        }

        // Check that withdrawal doesn't push utilization above max
        let new_total_deposits = total_deposits - usdc_to_return;
        if new_total_deposits > 0 {
            let new_util = shares::calc_utilization_bps(locked, new_total_deposits);
            if new_util > storage::get_max_utilization(&env) {
                return Err(PerpsError::MaxUtilizationExceeded);
            }
        } else if locked > 0 {
            return Err(PerpsError::InsufficientLiquidity);
        }

        // Burn shares
        storage::set_total_shares(&env, total_shares - share_amount);
        storage::set_total_deposits(&env, new_total_deposits);
        storage::set_share_balance(&env, &depositor, current_balance - share_amount);

        // Transfer USDC to depositor
        let usdc = storage::get_usdc_token(&env);
        let token_client = token::TokenClient::new(&env, &usdc);
        token_client.transfer(&env.current_contract_address(), &depositor, &usdc_to_return);

        events::emit_withdraw(&env, depositor, share_amount, usdc_to_return);

        Ok(usdc_to_return)
    }

    // -----------------------------------------------------------------------
    // Position manager operations (restricted)
    // -----------------------------------------------------------------------

    fn require_position_manager(env: &Env, caller: &Address) -> Result<(), PerpsError> {
        caller.require_auth();
        if !storage::has_position_manager(env) {
            return Err(PerpsError::NotInitialized);
        }
        let pm = storage::get_position_manager(env);
        if *caller != pm {
            return Err(PerpsError::Unauthorized);
        }
        Ok(())
    }

    /// Lock liquidity for a new position. Only callable by position manager.
    pub fn lock_liquidity(env: Env, caller: Address, amount: i128) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        Self::require_position_manager(&env, &caller)?;

        if amount <= 0 {
            return Err(PerpsError::InvalidAmount);
        }

        let total_deposits = storage::get_total_deposits(&env);
        let locked = storage::get_locked_liquidity(&env);
        let new_locked = locked + amount;

        let new_util = shares::calc_utilization_bps(new_locked, total_deposits);
        if new_util > storage::get_max_utilization(&env) {
            return Err(PerpsError::MaxUtilizationExceeded);
        }

        storage::set_locked_liquidity(&env, new_locked);
        events::emit_liquidity_locked(&env, amount);

        Ok(())
    }

    /// Unlock liquidity when a position closes. Only callable by position manager.
    pub fn unlock_liquidity(env: Env, caller: Address, amount: i128) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        Self::require_position_manager(&env, &caller)?;

        if amount <= 0 {
            return Err(PerpsError::InvalidAmount);
        }

        let locked = storage::get_locked_liquidity(&env);
        let new_locked = locked - amount;
        if new_locked < 0 {
            return Err(PerpsError::OverflowError);
        }

        storage::set_locked_liquidity(&env, new_locked);
        events::emit_liquidity_unlocked(&env, amount);

        Ok(())
    }

    /// Settle PnL for a closed position. Only callable by position manager.
    ///
    /// - pnl > 0: trader won → vault pays trader
    /// - pnl < 0: trader lost → vault received USDC, increase deposits
    pub fn settle_pnl(
        env: Env,
        caller: Address,
        trader: Address,
        pnl: i128,
    ) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        Self::require_position_manager(&env, &caller)?;

        let total_deposits = storage::get_total_deposits(&env);

        if pnl > 0 {
            // Trader won — vault pays out
            if pnl > total_deposits {
                return Err(PerpsError::InsufficientLiquidity);
            }
            let usdc = storage::get_usdc_token(&env);
            let token_client = token::TokenClient::new(&env, &usdc);
            token_client.transfer(&env.current_contract_address(), &trader, &pnl);
            storage::set_total_deposits(&env, total_deposits - pnl);
        } else if pnl < 0 {
            // Trader lost — USDC already transferred to vault by position manager
            storage::set_total_deposits(&env, total_deposits + pnl.abs());
        }
        // pnl == 0: no-op

        events::emit_pnl_settled(&env, trader, pnl);

        Ok(())
    }

    /// Collect trading fee. Only callable by position manager.
    /// Fee USDC has already been transferred to the vault.
    pub fn collect_fee(env: Env, caller: Address, amount: i128) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        Self::require_position_manager(&env, &caller)?;

        if amount <= 0 {
            return Err(PerpsError::InvalidAmount);
        }

        let total_deposits = storage::get_total_deposits(&env);
        storage::set_total_deposits(&env, total_deposits + amount);

        events::emit_fee_collected(&env, amount);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // View functions
    // -----------------------------------------------------------------------

    pub fn get_share_balance(env: Env, account: Address) -> i128 {
        storage::bump_instance(&env);
        storage::get_share_balance(&env, &account)
    }

    pub fn get_total_shares(env: Env) -> i128 {
        storage::bump_instance(&env);
        storage::get_total_shares(&env)
    }

    pub fn get_total_deposits(env: Env) -> i128 {
        storage::bump_instance(&env);
        storage::get_total_deposits(&env)
    }

    pub fn get_locked_liquidity(env: Env) -> i128 {
        storage::bump_instance(&env);
        storage::get_locked_liquidity(&env)
    }

    pub fn get_available_liquidity(env: Env) -> i128 {
        storage::bump_instance(&env);
        let total = storage::get_total_deposits(&env);
        let locked = storage::get_locked_liquidity(&env);
        total - locked
    }

    pub fn get_share_price(env: Env) -> i128 {
        storage::bump_instance(&env);
        let total_shares = storage::get_total_shares(&env);
        let total_deposits = storage::get_total_deposits(&env);
        shares::calc_share_price(total_shares, total_deposits)
    }

    pub fn get_utilization(env: Env) -> u32 {
        storage::bump_instance(&env);
        let total_deposits = storage::get_total_deposits(&env);
        let locked = storage::get_locked_liquidity(&env);
        shares::calc_utilization_bps(locked, total_deposits)
    }

    // -----------------------------------------------------------------------
    // Admin functions
    // -----------------------------------------------------------------------

    pub fn set_position_manager(
        env: Env,
        admin: Address,
        pm_address: Address,
    ) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        let stored_admin = storage::get_admin(&env);
        if admin != stored_admin {
            return Err(PerpsError::Unauthorized);
        }
        storage::set_position_manager(&env, &pm_address);
        Ok(())
    }

    pub fn set_max_utilization(env: Env, admin: Address, max_bps: u32) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        let stored_admin = storage::get_admin(&env);
        if admin != stored_admin {
            return Err(PerpsError::Unauthorized);
        }
        if max_bps == 0 || max_bps > BPS_DENOMINATOR {
            return Err(PerpsError::InvalidConfig);
        }
        storage::set_max_utilization(&env, max_bps);
        Ok(())
    }
}
