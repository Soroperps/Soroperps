#![no_std]

#[cfg(test)]
mod test;

use perps_types::{FundingKey, PerpsError, INSTANCE_TTL_EXTEND, INSTANCE_TTL_THRESHOLD};
use soroban_sdk::{contract, contractimpl, Address, Env};

use perps_position_manager::PositionManagerContractClient;

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

mod storage {
    use super::*;

    pub fn bump_instance(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND);
    }

    pub fn set_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&FundingKey::Admin, admin);
    }

    pub fn get_admin(env: &Env) -> Address {
        env.storage().instance().get(&FundingKey::Admin).unwrap()
    }

    pub fn has_admin(env: &Env) -> bool {
        env.storage().instance().has(&FundingKey::Admin)
    }

    pub fn set_position_manager(env: &Env, pm: &Address) {
        env.storage()
            .instance()
            .set(&FundingKey::PositionManagerAddress, pm);
    }

    pub fn get_position_manager(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&FundingKey::PositionManagerAddress)
            .unwrap()
    }

    pub fn set_funding_interval(env: &Env, interval: u64) {
        env.storage()
            .instance()
            .set(&FundingKey::FundingInterval, &interval);
    }

    pub fn get_funding_interval(env: &Env) -> u64 {
        env.storage()
            .instance()
            .get(&FundingKey::FundingInterval)
            .unwrap_or(3600) // default 1 hour
    }

    pub fn set_max_funding_rate(env: &Env, bps: u32) {
        env.storage()
            .instance()
            .set(&FundingKey::MaxFundingRate, &bps);
    }

    pub fn get_max_funding_rate(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&FundingKey::MaxFundingRate)
            .unwrap_or(10) // default 0.1% per interval
    }

    pub fn set_funding_spread(env: &Env, bps: u32) {
        env.storage()
            .instance()
            .set(&FundingKey::FundingSpread, &bps);
    }

    pub fn get_funding_spread(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&FundingKey::FundingSpread)
            .unwrap_or(5) // default 0.05%
    }

    pub fn set_cumulative_funding_long(env: &Env, index: i128) {
        env.storage()
            .instance()
            .set(&FundingKey::CumulativeFundingLong, &index);
    }

    pub fn get_cumulative_funding_long(env: &Env) -> i128 {
        env.storage()
            .instance()
            .get(&FundingKey::CumulativeFundingLong)
            .unwrap_or(0)
    }

    pub fn set_cumulative_funding_short(env: &Env, index: i128) {
        env.storage()
            .instance()
            .set(&FundingKey::CumulativeFundingShort, &index);
    }

    pub fn get_cumulative_funding_short(env: &Env) -> i128 {
        env.storage()
            .instance()
            .get(&FundingKey::CumulativeFundingShort)
            .unwrap_or(0)
    }

    pub fn set_last_funding_time(env: &Env, time: u64) {
        env.storage()
            .instance()
            .set(&FundingKey::LastFundingTime, &time);
    }

    pub fn get_last_funding_time(env: &Env) -> u64 {
        env.storage()
            .instance()
            .get(&FundingKey::LastFundingTime)
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

mod events {
    use soroban_sdk::{Env, Symbol};

    pub fn emit_funding_applied(
        env: &Env,
        long_rate: i128,
        short_rate: i128,
        cumulative_long: i128,
        cumulative_short: i128,
        oi_long: i128,
        oi_short: i128,
        timestamp: u64,
    ) {
        env.events().publish(
            (Symbol::new(env, "funding_applied"),),
            (
                long_rate,
                short_rate,
                cumulative_long,
                cumulative_short,
                oi_long,
                oi_short,
                timestamp,
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct FundingRateContract;

#[contractimpl]
impl FundingRateContract {
    /// Initialize the funding rate contract.
    pub fn initialize(
        env: Env,
        admin: Address,
        position_manager: Address,
        funding_interval: u64,
        max_funding_rate_bps: u32,
        funding_spread_bps: u32,
    ) -> Result<(), PerpsError> {
        if storage::has_admin(&env) {
            return Err(PerpsError::AlreadyInitialized);
        }
        if funding_interval == 0 || max_funding_rate_bps == 0 {
            return Err(PerpsError::InvalidConfig);
        }

        storage::set_admin(&env, &admin);
        storage::set_position_manager(&env, &position_manager);
        storage::set_funding_interval(&env, funding_interval);
        storage::set_max_funding_rate(&env, max_funding_rate_bps);
        storage::set_funding_spread(&env, funding_spread_bps);
        storage::set_cumulative_funding_long(&env, 0);
        storage::set_cumulative_funding_short(&env, 0);
        storage::set_last_funding_time(&env, env.ledger().timestamp());
        storage::bump_instance(&env);

        Ok(())
    }

    /// Calculate the current funding rate based on OI skew.
    ///
    /// Returns `(long_rate, short_rate)` in basis points.
    /// - When longs dominate: long_rate > 0 (longs pay), short_rate < 0 (shorts receive)
    /// - When shorts dominate: long_rate < 0 (longs receive), short_rate > 0 (shorts pay)
    /// - Protocol takes a spread from the paying side
    pub fn get_current_rate(env: Env) -> Result<(i128, i128), PerpsError> {
        storage::bump_instance(&env);

        let pm_addr = storage::get_position_manager(&env);
        let pm = PositionManagerContractClient::new(&env, &pm_addr);

        let oi_long = pm.get_open_interest_long();
        let oi_short = pm.get_open_interest_short();
        let total_oi = oi_long + oi_short;

        if total_oi == 0 {
            return Ok((0, 0));
        }

        let max_rate = storage::get_max_funding_rate(&env) as i128;
        let spread = storage::get_funding_spread(&env) as i128;

        // Skew in basis points: positive = longs dominate
        // skew = (oi_long - oi_short) * 10000 / total_oi
        let skew = (oi_long - oi_short) * 10_000 / total_oi;

        // Base rate: proportional to skew, clamped to max
        let base_rate = clamp(skew, -max_rate, max_rate);

        // Long rate: base_rate + spread (longs always pay a bit more)
        // Short rate: -(base_rate - spread) (shorts always pay a bit more too)
        // When base_rate > 0: longs pay (base_rate + spread), shorts receive (base_rate - spread)
        // When base_rate < 0: shorts pay (|base_rate| + spread), longs receive (|base_rate| - spread)
        let long_rate = base_rate + spread;
        let short_rate = -(base_rate - spread);

        Ok((long_rate, short_rate))
    }

    /// Keeper calls this to apply funding. Checks that enough time has passed.
    pub fn apply_funding(env: Env, keeper: Address) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        keeper.require_auth();

        let now = env.ledger().timestamp();
        let last = storage::get_last_funding_time(&env);
        let interval = storage::get_funding_interval(&env);

        if now - last < interval {
            return Err(PerpsError::FundingTooEarly);
        }

        // Calculate current rates
        let (long_rate, short_rate) = Self::get_current_rate(env.clone())?;

        // Update cumulative indices
        let cum_long = storage::get_cumulative_funding_long(&env) + long_rate;
        let cum_short = storage::get_cumulative_funding_short(&env) + short_rate;

        storage::set_cumulative_funding_long(&env, cum_long);
        storage::set_cumulative_funding_short(&env, cum_short);
        storage::set_last_funding_time(&env, now);

        // Get OI for event
        let pm_addr = storage::get_position_manager(&env);
        let pm = PositionManagerContractClient::new(&env, &pm_addr);
        let oi_long = pm.get_open_interest_long();
        let oi_short = pm.get_open_interest_short();

        events::emit_funding_applied(
            &env, long_rate, short_rate, cum_long, cum_short, oi_long, oi_short, now,
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // View functions
    // -----------------------------------------------------------------------

    pub fn get_cumulative_funding_long(env: Env) -> i128 {
        storage::bump_instance(&env);
        storage::get_cumulative_funding_long(&env)
    }

    pub fn get_cumulative_funding_short(env: Env) -> i128 {
        storage::bump_instance(&env);
        storage::get_cumulative_funding_short(&env)
    }

    pub fn get_last_funding_time(env: Env) -> u64 {
        storage::bump_instance(&env);
        storage::get_last_funding_time(&env)
    }

    // -----------------------------------------------------------------------
    // Admin functions
    // -----------------------------------------------------------------------

    pub fn set_funding_interval(env: Env, admin: Address, interval: u64) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        if admin != storage::get_admin(&env) {
            return Err(PerpsError::Unauthorized);
        }
        if interval == 0 {
            return Err(PerpsError::InvalidConfig);
        }
        storage::set_funding_interval(&env, interval);
        Ok(())
    }

    pub fn set_max_rate(env: Env, admin: Address, max_bps: u32) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        if admin != storage::get_admin(&env) {
            return Err(PerpsError::Unauthorized);
        }
        if max_bps == 0 {
            return Err(PerpsError::InvalidConfig);
        }
        storage::set_max_funding_rate(&env, max_bps);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn clamp(value: i128, min: i128, max: i128) -> i128 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}
