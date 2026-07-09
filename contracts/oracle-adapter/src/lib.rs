#![no_std]

#[cfg(test)]
mod test;

use perps_types::{
    CachedPrice, OracleCacheKey, OracleKey, PerpsError, INSTANCE_TTL_EXTEND,
    INSTANCE_TTL_THRESHOLD,
};
use soroban_sdk::{contract, contractimpl, Address, Env};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

mod events {
    use soroban_sdk::{Env, Symbol};

    pub fn emit_price_fetched(env: &Env, asset: u32, price: i128, timestamp: u64) {
        env.events().publish(
            (Symbol::new(env, "price_fetched"), asset),
            (price, timestamp),
        );
    }
}

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
        env.storage().instance().set(&OracleKey::Admin, admin);
    }

    pub fn get_admin(env: &Env) -> Address {
        env.storage().instance().get(&OracleKey::Admin).unwrap()
    }

    pub fn has_admin(env: &Env) -> bool {
        env.storage().instance().has(&OracleKey::Admin)
    }

    pub fn set_staleness_threshold(env: &Env, threshold: u64) {
        env.storage()
            .instance()
            .set(&OracleKey::StalenessThreshold, &threshold);
    }

    pub fn get_staleness_threshold(env: &Env) -> u64 {
        env.storage()
            .instance()
            .get(&OracleKey::StalenessThreshold)
            .unwrap_or(300) // default 5 minutes
    }

    pub fn set_cached_price(env: &Env, asset: u32, price: &CachedPrice) {
        let key = OracleCacheKey::Price(asset);
        env.storage().temporary().set(&key, price);
        // Temporary storage: short TTL, auto-expires
        env.storage().temporary().extend_ttl(&key, 100, 200);
    }

    pub fn get_cached_price(env: &Env, asset: u32) -> Option<CachedPrice> {
        let key = OracleCacheKey::Price(asset);
        env.storage().temporary().get(&key)
    }
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct OracleAdapterContract;

#[contractimpl]
impl OracleAdapterContract {
    /// Initialize the oracle adapter.
    pub fn initialize(
        env: Env,
        admin: Address,
        staleness_threshold: u64,
    ) -> Result<(), PerpsError> {
        if storage::has_admin(&env) {
            return Err(PerpsError::AlreadyInitialized);
        }
        if staleness_threshold == 0 {
            return Err(PerpsError::InvalidConfig);
        }

        storage::set_admin(&env, &admin);
        storage::set_staleness_threshold(&env, staleness_threshold);
        storage::bump_instance(&env);

        Ok(())
    }

    /// Admin sets a price for an asset. This is the MVP approach.
    /// In production, this would be replaced by a Reflector cross-contract call.
    /// The `asset` is identified by a u32 index (e.g., 0=XLM, 1=BTC, 2=ETH).
    pub fn set_price(
        env: Env,
        admin: Address,
        asset: u32,
        price: i128,
    ) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        let stored_admin = storage::get_admin(&env);
        if admin != stored_admin {
            return Err(PerpsError::Unauthorized);
        }
        if price <= 0 {
            return Err(PerpsError::InvalidAmount);
        }

        let cached = CachedPrice {
            price,
            timestamp: env.ledger().timestamp(),
        };
        storage::set_cached_price(&env, asset, &cached);
        events::emit_price_fetched(&env, asset, price, cached.timestamp);

        Ok(())
    }

    /// Get the latest price for an asset, checking staleness.
    /// Returns (price, timestamp). Errors if price is stale or missing.
    pub fn get_price(env: Env, asset: u32) -> Result<CachedPrice, PerpsError> {
        storage::bump_instance(&env);

        let cached = storage::get_cached_price(&env, asset)
            .ok_or(PerpsError::StalePrice)?;

        let threshold = storage::get_staleness_threshold(&env);
        let now = env.ledger().timestamp();

        if now - cached.timestamp > threshold {
            return Err(PerpsError::StalePrice);
        }

        Ok(cached)
    }

    /// Get price value only. Convenience wrapper.
    pub fn get_price_value(env: Env, asset: u32) -> Result<i128, PerpsError> {
        let cached = Self::get_price(env, asset)?;
        Ok(cached.price)
    }

    // -----------------------------------------------------------------------
    // Admin
    // -----------------------------------------------------------------------

    pub fn set_staleness_threshold(
        env: Env,
        admin: Address,
        threshold: u64,
    ) -> Result<(), PerpsError> {
        storage::bump_instance(&env);
        admin.require_auth();
        let stored_admin = storage::get_admin(&env);
        if admin != stored_admin {
            return Err(PerpsError::Unauthorized);
        }
        if threshold == 0 {
            return Err(PerpsError::InvalidConfig);
        }
        storage::set_staleness_threshold(&env, threshold);
        Ok(())
    }
}
