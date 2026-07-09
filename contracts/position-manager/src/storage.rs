use soroban_sdk::{Address, Env};

use perps_types::{
    Position, PositionKey, INSTANCE_TTL_EXTEND, INSTANCE_TTL_THRESHOLD, POSITION_TTL_EXTEND,
    POSITION_TTL_THRESHOLD,
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
    env.storage().instance().set(&PositionKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&PositionKey::Admin).unwrap()
}

pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&PositionKey::Admin)
}

pub fn set_vault(env: &Env, vault: &Address) {
    env.storage()
        .instance()
        .set(&PositionKey::VaultAddress, vault);
}

pub fn get_vault(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&PositionKey::VaultAddress)
        .unwrap()
}

pub fn set_oracle(env: &Env, oracle: &Address) {
    env.storage()
        .instance()
        .set(&PositionKey::OracleAddress, oracle);
}

pub fn get_oracle(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&PositionKey::OracleAddress)
        .unwrap()
}

pub fn set_funding(env: &Env, funding: &Address) {
    env.storage()
        .instance()
        .set(&PositionKey::FundingAddress, funding);
}

#[allow(dead_code)]
pub fn get_funding(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .get(&PositionKey::FundingAddress)
}

pub fn set_liquidation_engine(env: &Env, le: &Address) {
    env.storage()
        .instance()
        .set(&PositionKey::LiquidationEngine, le);
}

pub fn get_liquidation_engine(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .get(&PositionKey::LiquidationEngine)
}

pub fn set_usdc_token(env: &Env, token: &Address) {
    env.storage()
        .instance()
        .set(&PositionKey::UsdcToken, token);
}

pub fn get_usdc_token(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&PositionKey::UsdcToken)
        .unwrap()
}

pub fn set_open_fee_rate(env: &Env, bps: u32) {
    env.storage()
        .instance()
        .set(&PositionKey::OpenFeeRate, &bps);
}

pub fn get_open_fee_rate(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&PositionKey::OpenFeeRate)
        .unwrap_or(10) // default 0.1%
}

pub fn set_close_fee_rate(env: &Env, bps: u32) {
    env.storage()
        .instance()
        .set(&PositionKey::CloseFeeRate, &bps);
}

pub fn get_close_fee_rate(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&PositionKey::CloseFeeRate)
        .unwrap_or(10)
}

pub fn set_max_leverage(env: &Env, max: u32) {
    env.storage()
        .instance()
        .set(&PositionKey::MaxLeverage, &max);
}

pub fn get_max_leverage(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&PositionKey::MaxLeverage)
        .unwrap_or(30)
}

pub fn set_min_collateral(env: &Env, min: i128) {
    env.storage()
        .instance()
        .set(&PositionKey::MinCollateral, &min);
}

pub fn get_min_collateral(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&PositionKey::MinCollateral)
        .unwrap_or(10_0000000) // default 10 USDC
}

// ---------------------------------------------------------------------------
// Position counter
// ---------------------------------------------------------------------------

pub fn get_next_position_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&PositionKey::NextPositionId)
        .unwrap_or(1)
}

pub fn increment_position_id(env: &Env) -> u64 {
    let id = get_next_position_id(env);
    env.storage()
        .instance()
        .set(&PositionKey::NextPositionId, &(id + 1));
    id
}

// ---------------------------------------------------------------------------
// Open interest tracking (Instance storage)
// ---------------------------------------------------------------------------

pub fn get_oi_long(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&PositionKey::OpenInterestLong)
        .unwrap_or(0)
}

pub fn set_oi_long(env: &Env, oi: i128) {
    env.storage()
        .instance()
        .set(&PositionKey::OpenInterestLong, &oi);
}

pub fn get_oi_short(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&PositionKey::OpenInterestShort)
        .unwrap_or(0)
}

pub fn set_oi_short(env: &Env, oi: i128) {
    env.storage()
        .instance()
        .set(&PositionKey::OpenInterestShort, &oi);
}

// ---------------------------------------------------------------------------
// Per-position storage (Persistent)
// ---------------------------------------------------------------------------

pub fn save_position(env: &Env, trader: &Address, id: u64, position: &Position) {
    let key = PositionKey::Position(trader.clone(), id);
    env.storage().persistent().set(&key, position);
    env.storage()
        .persistent()
        .extend_ttl(&key, POSITION_TTL_THRESHOLD, POSITION_TTL_EXTEND);
}

pub fn get_position(env: &Env, trader: &Address, id: u64) -> Option<Position> {
    let key = PositionKey::Position(trader.clone(), id);
    let pos: Option<Position> = env.storage().persistent().get(&key);
    if pos.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, POSITION_TTL_THRESHOLD, POSITION_TTL_EXTEND);
    }
    pos
}

pub fn remove_position(env: &Env, trader: &Address, id: u64) {
    let key = PositionKey::Position(trader.clone(), id);
    env.storage().persistent().remove(&key);
}

// ---------------------------------------------------------------------------
// Per-trader position count
// ---------------------------------------------------------------------------

pub fn get_trader_position_count(env: &Env, trader: &Address) -> u64 {
    let key = PositionKey::TraderPositionCount(trader.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub fn set_trader_position_count(env: &Env, trader: &Address, count: u64) {
    let key = PositionKey::TraderPositionCount(trader.clone());
    env.storage().persistent().set(&key, &count);
}
