#![no_std]

use soroban_sdk::{contracttype, contracterror, Address};

// ---------------------------------------------------------------------------
// Vault storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum VaultKey {
    Admin,
    UsdcToken,
    TotalShares,
    TotalDeposits,
    LockedLiquidity,
    MaxUtilization,
    PositionManager,
    ShareBalance(Address),
}

// ---------------------------------------------------------------------------
// Position types
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Direction {
    Long = 0,
    Short = 1,
}

#[contracttype]
#[derive(Clone)]
pub struct Position {
    pub id: u64,
    pub trader: Address,
    pub asset: u32,
    pub direction: Direction,
    pub size: i128,
    pub collateral: i128,
    pub entry_price: i128,
    pub leverage: u32,
    pub funding_index: i128,
    pub opened_at: u64,
}

// ---------------------------------------------------------------------------
// Position manager storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum PositionKey {
    Admin,
    VaultAddress,
    OracleAddress,
    FundingAddress,
    LiquidationEngine,
    UsdcToken,
    NextPositionId,
    OpenFeeRate,
    CloseFeeRate,
    MaxLeverage,
    MinCollateral,
    OpenInterestLong,
    OpenInterestShort,
    Position(Address, u64),
    TraderPositionCount(Address),
}

// ---------------------------------------------------------------------------
// Oracle adapter storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum OracleKey {
    Admin,
    ReflectorAddress,
    StalenessThreshold,
}

#[contracttype]
#[derive(Clone)]
pub struct CachedPrice {
    pub price: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum OracleCacheKey {
    Price(u32),
}

// ---------------------------------------------------------------------------
// Funding rate storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum FundingKey {
    Admin,
    PositionManagerAddress,
    FundingInterval,
    MaxFundingRate,
    FundingSpread,
    CumulativeFundingLong,
    CumulativeFundingShort,
    LastFundingTime,
}

// ---------------------------------------------------------------------------
// Liquidation engine storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum LiquidationKey {
    Admin,
    PositionManagerAddress,
    OracleAddress,
    MaintenanceMarginRate,
    LiquidationPenalty,
    KeeperReward,
}

// ---------------------------------------------------------------------------
// Shared error type
// ---------------------------------------------------------------------------

#[contracterror]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum PerpsError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    InsufficientBalance = 3,
    InsufficientLiquidity = 4,
    InvalidAmount = 5,
    InvalidLeverage = 6,
    PositionNotFound = 7,
    StalePrice = 8,
    MaxUtilizationExceeded = 9,
    NotLiquidatable = 10,
    Unauthorized = 11,
    InvalidConfig = 12,
    OverflowError = 13,
    ZeroShares = 14,
    MinCollateralNotMet = 15,
    FundingTooEarly = 16,
    AssetMismatch = 17,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const DECIMALS: i128 = 10_000_000; // 7 decimal places
pub const BPS_DENOMINATOR: u32 = 10_000;

// TTL constants (in ledgers, ~5s each)
pub const POSITION_TTL_THRESHOLD: u32 = 518_400; // ~30 days
pub const POSITION_TTL_EXTEND: u32 = 1_036_800; // ~60 days
pub const BALANCE_TTL_THRESHOLD: u32 = 518_400;
pub const BALANCE_TTL_EXTEND: u32 = 1_036_800;
pub const INSTANCE_TTL_THRESHOLD: u32 = 17_280; // ~1 day
pub const INSTANCE_TTL_EXTEND: u32 = 518_400; // ~30 days
