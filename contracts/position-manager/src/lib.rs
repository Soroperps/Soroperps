#![no_std]

pub mod math;
mod storage;
#[cfg(test)]
mod test;

use perps_types::{Direction, PerpsError, Position};
use soroban_sdk::{contract, contractimpl, token, Address, Env};

use perps_oracle_adapter::OracleAdapterContractClient;
use perps_vault::VaultContractClient;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

mod events {
    use perps_types::Direction;
    use soroban_sdk::{Address, Env, Symbol};

    pub fn emit_position_opened(
        env: &Env,
        trader: &Address,
        position_id: u64,
        direction: &Direction,
        size: i128,
        collateral: i128,
        entry_price: i128,
        leverage: u32,
        fee: i128,
    ) {
        env.events().publish(
            (Symbol::new(env, "position_opened"), trader.clone(), position_id),
            (direction.clone(), size, collateral, entry_price, leverage, fee),
        );
    }

    pub fn emit_position_closed(
        env: &Env,
        trader: &Address,
        position_id: u64,
        exit_price: i128,
        realized_pnl: i128,
        fee: i128,
    ) {
        env.events().publish(
            (Symbol::new(env, "position_closed"), trader.clone(), position_id),
            (exit_price, realized_pnl, fee),
        );
    }
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct PositionManagerContract;

#[contractimpl]
impl PositionManagerContract {
    /// Initialize the position manager.
    pub fn initialize(
        env: Env,
        admin: Address,
        vault: Address,
        oracle: Address,
        usdc_token: Address,
        open_fee_bps: u32,
        close_fee_bps: u32,
        max_leverage: u32,
        min_collateral: i128,
    ) -> Result<(), PerpsError> {
        if storage::has_admin(&env) {
            return Err(PerpsError::AlreadyInitialized);
        }
        if max_leverage == 0 || open_fee_bps > 1000 || close_fee_bps > 1000 {
            return Err(PerpsError::InvalidConfig);
        }

        storage::set_admin(&env, &admin);
        storage::set_vault(&env, &vault);
        storage::set_oracle(&env, &oracle);
        storage::set_usdc_token(&env, &usdc_token);
        storage::set_open_fee_rate(&env, open_fee_bps);
        storage::set_close_fee_rate(&env, close_fee_bps);
        storage::set_max_leverage(&env, max_leverage);
        storage::set_min_collateral(&env, min_collateral);
        storage::set_oi_long(&env, 0);
        storage::set_oi_short(&env, 0);
        storage::bump_instance(&env);

        Ok(())
    }

    /// Open a new leveraged position.
    pub fn open_position(
        env: Env,
        trader: Address,
        asset: u32,
        direction: Direction,
        collateral: i128,
        leverage: u32,
    ) -> Result<u64, PerpsError> {
        storage::bump_instance(&env);
        trader.require_auth();

        // Validate inputs
        if collateral < storage::get_min_collateral(&env) {
            return Err(PerpsError::MinCollateralNotMet);
        }
        if leverage == 0 || leverage > storage::get_max_leverage(&env) {
            return Err(PerpsError::InvalidLeverage);
        }

        // Get current oracle price
        let oracle_addr = storage::get_oracle(&env);
        let oracle = OracleAdapterContractClient::new(&env, &oracle_addr);
        let entry_price = oracle.get_price_value(&asset);

        // Calculate notional size and fees
        let size = collateral * (leverage as i128);
        let open_fee = math::calculate_fee(size, storage::get_open_fee_rate(&env));
        let total_cost = collateral + open_fee;

        // Transfer USDC from trader to vault
        let usdc_addr = storage::get_usdc_token(&env);
        let vault_addr = storage::get_vault(&env);
        let token_client = token::TokenClient::new(&env, &usdc_addr);
        token_client.transfer(&trader, &vault_addr, &total_cost);

        // Lock liquidity in the vault (for potential payout)
        let vault = VaultContractClient::new(&env, &vault_addr);
        vault.lock_liquidity(&env.current_contract_address(), &size);

        // Collect the fee
        vault.collect_fee(&env.current_contract_address(), &open_fee);

        // Create position
        let position_id = storage::increment_position_id(&env);
        let position = Position {
            id: position_id,
            trader: trader.clone(),
            direction: direction.clone(),
            size,
            collateral,
            entry_price,
            leverage,
            funding_index: 0, // Will be set properly when funding contract is connected
            opened_at: env.ledger().timestamp(),
        };

        storage::save_position(&env, &trader, position_id, &position);

        // Update open interest
        match direction {
            Direction::Long => {
                storage::set_oi_long(&env, storage::get_oi_long(&env) + size);
            }
            Direction::Short => {
                storage::set_oi_short(&env, storage::get_oi_short(&env) + size);
            }
        }

        // Update trader position count
        let count = storage::get_trader_position_count(&env, &trader);
        storage::set_trader_position_count(&env, &trader, count + 1);

        events::emit_position_opened(
            &env,
            &trader,
            position_id,
            &direction,
            size,
            collateral,
            entry_price,
            leverage,
            open_fee,
        );

        Ok(position_id)
    }

    /// Close a position and settle PnL.
    pub fn close_position(
        env: Env,
        trader: Address,
        position_id: u64,
        asset: u32,
    ) -> Result<i128, PerpsError> {
        storage::bump_instance(&env);
        trader.require_auth();

        let position = storage::get_position(&env, &trader, position_id)
            .ok_or(PerpsError::PositionNotFound)?;

        if position.trader != trader {
            return Err(PerpsError::Unauthorized);
        }

        // Get current price
        let oracle_addr = storage::get_oracle(&env);
        let oracle = OracleAdapterContractClient::new(&env, &oracle_addr);
        let current_price = oracle.get_price_value(&asset);

        // Calculate PnL
        let price_pnl = math::calculate_pnl(
            &position.direction,
            position.size,
            position.entry_price,
            current_price,
        );

        // Calculate close fee
        let close_fee = math::calculate_fee(position.size, storage::get_close_fee_rate(&env));

        // Net PnL after fees (funding will be added in Phase 4)
        let net_pnl = price_pnl - close_fee;

        // Single vault call: unlock, adjust accounting, return funds to trader
        let vault_addr = storage::get_vault(&env);
        let vault = VaultContractClient::new(&env, &vault_addr);
        vault.close_position_settlement(
            &env.current_contract_address(),
            &trader,
            &position.size,
            &position.collateral,
            &price_pnl,
            &close_fee,
        );

        // Remove position
        storage::remove_position(&env, &trader, position_id);

        // Update open interest
        match position.direction {
            Direction::Long => {
                storage::set_oi_long(&env, storage::get_oi_long(&env) - position.size);
            }
            Direction::Short => {
                storage::set_oi_short(&env, storage::get_oi_short(&env) - position.size);
            }
        }

        // Update trader position count
        let count = storage::get_trader_position_count(&env, &trader);
        if count > 0 {
            storage::set_trader_position_count(&env, &trader, count - 1);
        }

        events::emit_position_closed(&env, &trader, position_id, current_price, net_pnl, close_fee);

        Ok(net_pnl)
    }

    /// Get a position's details.
    pub fn get_position(
        env: Env,
        trader: Address,
        position_id: u64,
    ) -> Result<Position, PerpsError> {
        storage::bump_instance(&env);
        storage::get_position(&env, &trader, position_id).ok_or(PerpsError::PositionNotFound)
    }

    /// Calculate unrealized PnL for a position at current oracle price.
    pub fn get_unrealized_pnl(
        env: Env,
        trader: Address,
        position_id: u64,
        asset: u32,
    ) -> Result<i128, PerpsError> {
        storage::bump_instance(&env);

        let position = storage::get_position(&env, &trader, position_id)
            .ok_or(PerpsError::PositionNotFound)?;

        let oracle_addr = storage::get_oracle(&env);
        let oracle = OracleAdapterContractClient::new(&env, &oracle_addr);
        let current_price = oracle.get_price_value(&asset);

        let pnl = math::calculate_pnl(
            &position.direction,
            position.size,
            position.entry_price,
            current_price,
        );

        Ok(pnl)
    }

    /// Force-close a position (called by liquidation engine only).
    pub fn liquidate_position(
        env: Env,
        caller: Address,
        trader: Address,
        position_id: u64,
        asset: u32,
    ) -> Result<(i128, i128), PerpsError> {
        storage::bump_instance(&env);
        caller.require_auth();

        // Only liquidation engine can call this
        let le = storage::get_liquidation_engine(&env)
            .ok_or(PerpsError::NotInitialized)?;
        if caller != le {
            return Err(PerpsError::Unauthorized);
        }

        let position = storage::get_position(&env, &trader, position_id)
            .ok_or(PerpsError::PositionNotFound)?;

        // Get current price
        let oracle_addr = storage::get_oracle(&env);
        let oracle = OracleAdapterContractClient::new(&env, &oracle_addr);
        let current_price = oracle.get_price_value(&asset);

        // Calculate PnL
        let price_pnl = math::calculate_pnl(
            &position.direction,
            position.size,
            position.entry_price,
            current_price,
        );

        // Settlement: unlock liquidity, no fee on liquidation (penalty handled by liquidation engine)
        let vault_addr = storage::get_vault(&env);
        let vault = VaultContractClient::new(&env, &vault_addr);
        vault.close_position_settlement(
            &env.current_contract_address(),
            &trader,
            &position.size,
            &position.collateral,
            &price_pnl,
            &0_i128, // no trading fee on liquidation
        );

        // Remove position
        storage::remove_position(&env, &trader, position_id);

        // Update open interest
        match position.direction {
            Direction::Long => {
                storage::set_oi_long(&env, storage::get_oi_long(&env) - position.size);
            }
            Direction::Short => {
                storage::set_oi_short(&env, storage::get_oi_short(&env) - position.size);
            }
        }

        let count = storage::get_trader_position_count(&env, &trader);
        if count > 0 {
            storage::set_trader_position_count(&env, &trader, count - 1);
        }

        // Return (collateral, pnl) for liquidation engine to distribute
        Ok((position.collateral, price_pnl))
    }

    // -----------------------------------------------------------------------
    // View functions
    // -----------------------------------------------------------------------

    pub fn get_open_interest_long(env: Env) -> i128 {
        storage::bump_instance(&env);
        storage::get_oi_long(&env)
    }

    pub fn get_open_interest_short(env: Env) -> i128 {
        storage::bump_instance(&env);
        storage::get_oi_short(&env)
    }

    pub fn get_trader_position_count(env: Env, trader: Address) -> u64 {
        storage::bump_instance(&env);
        storage::get_trader_position_count(&env, &trader)
    }

    // -----------------------------------------------------------------------
    // Admin functions
    // -----------------------------------------------------------------------

    pub fn set_fee_rates(
        env: Env,
        admin: Address,
        open_bps: u32,
        close_bps: u32,
    ) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        if admin != storage::get_admin(&env) {
            return Err(PerpsError::Unauthorized);
        }
        storage::set_open_fee_rate(&env, open_bps);
        storage::set_close_fee_rate(&env, close_bps);
        Ok(())
    }

    pub fn set_max_leverage(env: Env, admin: Address, max: u32) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        if admin != storage::get_admin(&env) {
            return Err(PerpsError::Unauthorized);
        }
        if max == 0 {
            return Err(PerpsError::InvalidConfig);
        }
        storage::set_max_leverage(&env, max);
        Ok(())
    }

    pub fn set_liquidation_engine(
        env: Env,
        admin: Address,
        le_address: Address,
    ) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        if admin != storage::get_admin(&env) {
            return Err(PerpsError::Unauthorized);
        }
        storage::set_liquidation_engine(&env, &le_address);
        Ok(())
    }

    pub fn set_funding_address(
        env: Env,
        admin: Address,
        funding: Address,
    ) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        if admin != storage::get_admin(&env) {
            return Err(PerpsError::Unauthorized);
        }
        storage::set_funding(&env, &funding);
        Ok(())
    }
}
