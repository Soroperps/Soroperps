use soroban_sdk::{Address, Env};

use perps_types::{
    VaultKey, BALANCE_TTL_EXTEND, BALANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND,
    INSTANCE_TTL_THRESHOLD,
};

// ---------------------------------------------------------------------------
// Instance storage helpers (contract config)
// ---------------------------------------------------------------------------

pub fn bump_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND);
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&VaultKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&VaultKey::Admin).unwrap()
}

pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&VaultKey::Admin)
}

pub fn set_usdc_token(env: &Env, token: &Address) {
    env.storage().instance().set(&VaultKey::UsdcToken, token);
}

pub fn get_usdc_token(env: &Env) -> Address {
    env.storage().instance().get(&VaultKey::UsdcToken).unwrap()
}

pub fn set_total_shares(env: &Env, shares: i128) {
    env.storage()
        .instance()
        .set(&VaultKey::TotalShares, &shares);
}

pub fn get_total_shares(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&VaultKey::TotalShares)
        .unwrap_or(0)
}

pub fn set_total_deposits(env: &Env, deposits: i128) {
    env.storage()
        .instance()
        .set(&VaultKey::TotalDeposits, &deposits);
}

pub fn get_total_deposits(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&VaultKey::TotalDeposits)
        .unwrap_or(0)
}

pub fn set_locked_liquidity(env: &Env, locked: i128) {
    env.storage()
        .instance()
        .set(&VaultKey::LockedLiquidity, &locked);
}

pub fn get_locked_liquidity(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&VaultKey::LockedLiquidity)
        .unwrap_or(0)
}

pub fn set_max_utilization(env: &Env, max_bps: u32) {
    env.storage()
        .instance()
        .set(&VaultKey::MaxUtilization, &max_bps);
}

pub fn get_max_utilization(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&VaultKey::MaxUtilization)
        .unwrap_or(8000)
}

pub fn set_position_manager(env: &Env, pm: &Address) {
    env.storage()
        .instance()
        .set(&VaultKey::PositionManager, pm);
}

pub fn get_position_manager(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&VaultKey::PositionManager)
        .unwrap()
}

pub fn has_position_manager(env: &Env) -> bool {
    env.storage().instance().has(&VaultKey::PositionManager)
}

// ---------------------------------------------------------------------------
// Persistent storage helpers (per-LP share balances)
// ---------------------------------------------------------------------------

pub fn set_share_balance(env: &Env, account: &Address, balance: i128) {
    let key = VaultKey::ShareBalance(account.clone());
    env.storage().persistent().set(&key, &balance);
    env.storage()
        .persistent()
        .extend_ttl(&key, BALANCE_TTL_THRESHOLD, BALANCE_TTL_EXTEND);
}

pub fn get_share_balance(env: &Env, account: &Address) -> i128 {
    let key = VaultKey::ShareBalance(account.clone());
    let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    if balance > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BALANCE_TTL_THRESHOLD, BALANCE_TTL_EXTEND);
    }
    balance
}
