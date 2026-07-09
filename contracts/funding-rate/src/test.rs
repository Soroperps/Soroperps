use soroban_sdk::{testutils::Address as _, testutils::Ledger, token, Address, Env};

use crate::FundingRateContract;
use crate::FundingRateContractClient;
use perps_oracle_adapter::OracleAdapterContract;
use perps_oracle_adapter::OracleAdapterContractClient;
use perps_position_manager::PositionManagerContract;
use perps_position_manager::PositionManagerContractClient;
use perps_types::Direction;
use perps_vault::VaultContract;
use perps_vault::VaultContractClient;

struct TestSetup<'a> {
    env: Env,
    #[allow(dead_code)]
    admin: Address,
    usdc_address: Address,
    #[allow(dead_code)]
    vault: VaultContractClient<'a>,
    #[allow(dead_code)]
    oracle: OracleAdapterContractClient<'a>,
    pm: PositionManagerContractClient<'a>,
    funding: FundingRateContractClient<'a>,
}

fn setup() -> TestSetup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let usdc_admin = Address::generate(&env);
    let usdc_contract = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_address = usdc_contract.address();

    // Deploy vault
    let vault_address = env.register(VaultContract, ());
    let vault = VaultContractClient::new(&env, &vault_address);
    vault.initialize(&admin, &usdc_address, &8000_u32);

    // Deploy oracle
    let oracle_address = env.register(OracleAdapterContract, ());
    let oracle = OracleAdapterContractClient::new(&env, &oracle_address);
    oracle.initialize(&admin, &300_u64);

    // Deploy position manager
    let pm_address = env.register(PositionManagerContract, ());
    let pm = PositionManagerContractClient::new(&env, &pm_address);
    pm.initialize(
        &admin,
        &vault_address,
        &oracle_address,
        &usdc_address,
        &10_u32,
        &10_u32,
        &30_u32,
        &10_0000000_i128,
    );

    // Deploy funding rate
    let funding_address = env.register(FundingRateContract, ());
    let funding = FundingRateContractClient::new(&env, &funding_address);

    // Set timestamp before initializing funding (it captures init time)
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    funding.initialize(
        &admin,
        &pm_address,
        &3600_u64, // 1 hour interval
        &100_u32,  // max 1% per interval
        &5_u32,    // 0.05% spread
    );

    // Wire vault
    vault.set_position_manager(&admin, &pm_address);

    // Add LP liquidity
    let lp = Address::generate(&env);
    let lp_admin = token::StellarAssetClient::new(&env, &usdc_address);
    lp_admin.mint(&lp, &100_000_0000000_i128);
    vault.deposit(&lp, &100_000_0000000_i128);

    // Set oracle price
    oracle.set_price(&admin, &0_u32, &1_000_0000000_i128);

    TestSetup {
        env,
        admin,
        usdc_address,
        vault,
        oracle,
        pm,
        funding,
    }
}

fn create_trader(s: &TestSetup, amount: i128) -> Address {
    let trader = Address::generate(&s.env);
    let admin_client = token::StellarAssetClient::new(&s.env, &s.usdc_address);
    admin_client.mint(&trader, &amount);
    trader
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_zero_oi_zero_rate() {
    let s = setup();

    // No open positions, rates should be zero
    let (long_rate, short_rate) = s.funding.get_current_rate();
    assert_eq!(long_rate, 0);
    assert_eq!(short_rate, 0);
}

#[test]
fn test_balanced_oi_spread_only() {
    let s = setup();

    // Open equal long and short
    let t1 = create_trader(&s, 500_0000000);
    let t2 = create_trader(&s, 500_0000000);

    s.pm.open_position(&t1, &0_u32, &Direction::Long, &100_0000000_i128, &10_u32);
    s.pm.open_position(&t2, &0_u32, &Direction::Short, &100_0000000_i128, &10_u32);

    // Balanced OI: skew = 0, base_rate = 0
    // long_rate = 0 + 5 = 5 (spread only)
    // short_rate = -(0 - 5) = 5 (spread only)
    let (long_rate, short_rate) = s.funding.get_current_rate();
    assert_eq!(long_rate, 5); // both sides pay the spread
    assert_eq!(short_rate, 5);
}

#[test]
fn test_long_heavy_positive_rate() {
    let s = setup();

    // More longs than shorts
    let t1 = create_trader(&s, 500_0000000);
    let t2 = create_trader(&s, 500_0000000);

    s.pm.open_position(&t1, &0_u32, &Direction::Long, &200_0000000_i128, &10_u32); // 2000 notional
    s.pm.open_position(&t2, &0_u32, &Direction::Short, &100_0000000_i128, &10_u32); // 1000 notional

    // skew = (2000 - 1000) * 10000 / 3000 = 3333 bps
    // clamped to max 100 bps
    // long_rate = 100 + 5 = 105
    // short_rate = -(100 - 5) = -95 (shorts receive 95 bps)
    let (long_rate, short_rate) = s.funding.get_current_rate();
    assert_eq!(long_rate, 105);
    assert_eq!(short_rate, -95);
}

#[test]
fn test_short_heavy_negative_rate() {
    let s = setup();

    let t1 = create_trader(&s, 500_0000000);
    let t2 = create_trader(&s, 500_0000000);

    s.pm.open_position(&t1, &0_u32, &Direction::Long, &100_0000000_i128, &10_u32); // 1000
    s.pm.open_position(&t2, &0_u32, &Direction::Short, &200_0000000_i128, &10_u32); // 2000

    // skew = (1000 - 2000) * 10000 / 3000 = -3333 bps
    // clamped to -100 bps
    // long_rate = -100 + 5 = -95 (longs receive 95)
    // short_rate = -(-100 - 5) = 105 (shorts pay 105)
    let (long_rate, short_rate) = s.funding.get_current_rate();
    assert_eq!(long_rate, -95);
    assert_eq!(short_rate, 105);
}

#[test]
fn test_apply_funding_updates_indices() {
    let s = setup();

    let t1 = create_trader(&s, 500_0000000);
    let t2 = create_trader(&s, 500_0000000);

    s.pm.open_position(&t1, &0_u32, &Direction::Long, &200_0000000_i128, &10_u32);
    s.pm.open_position(&t2, &0_u32, &Direction::Short, &100_0000000_i128, &10_u32);

    assert_eq!(s.funding.get_cumulative_funding_long(), 0);
    assert_eq!(s.funding.get_cumulative_funding_short(), 0);

    // Advance time past interval
    s.env.ledger().with_mut(|li| {
        li.timestamp = 1000 + 3600; // 1 hour later
    });

    let keeper = Address::generate(&s.env);
    s.funding.apply_funding(&keeper);

    // Indices should be updated: long = 105, short = -95
    assert_eq!(s.funding.get_cumulative_funding_long(), 105);
    assert_eq!(s.funding.get_cumulative_funding_short(), -95);
    assert_eq!(s.funding.get_last_funding_time(), 4600);
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")]
fn test_apply_too_early() {
    let s = setup();

    let t1 = create_trader(&s, 500_0000000);
    s.pm.open_position(&t1, &0_u32, &Direction::Long, &100_0000000_i128, &10_u32);

    // Only advance 30 minutes (less than 1 hour interval)
    s.env.ledger().with_mut(|li| {
        li.timestamp = 1000 + 1800;
    });

    let keeper = Address::generate(&s.env);
    s.funding.apply_funding(&keeper);
}

#[test]
fn test_cumulative_indices_accumulate() {
    let s = setup();

    let t1 = create_trader(&s, 500_0000000);
    let t2 = create_trader(&s, 500_0000000);

    s.pm.open_position(&t1, &0_u32, &Direction::Long, &200_0000000_i128, &10_u32);
    s.pm.open_position(&t2, &0_u32, &Direction::Short, &100_0000000_i128, &10_u32);

    let keeper = Address::generate(&s.env);

    // First funding application
    s.env.ledger().with_mut(|li| {
        li.timestamp = 1000 + 3600;
    });
    s.funding.apply_funding(&keeper);
    assert_eq!(s.funding.get_cumulative_funding_long(), 105);

    // Second funding application
    s.env.ledger().with_mut(|li| {
        li.timestamp = 1000 + 7200;
    });
    s.funding.apply_funding(&keeper);
    assert_eq!(s.funding.get_cumulative_funding_long(), 210); // 105 + 105
    assert_eq!(s.funding.get_cumulative_funding_short(), -190); // -95 + -95
}

#[test]
fn test_max_rate_clamped() {
    let s = setup();

    // Only longs, no shorts — extreme skew
    let t1 = create_trader(&s, 500_0000000);
    s.pm.open_position(&t1, &0_u32, &Direction::Long, &100_0000000_i128, &10_u32);

    // skew = (1000 - 0) * 10000 / 1000 = 10000 bps
    // clamped to max 100 bps
    // long_rate = 100 + 5 = 105
    let (long_rate, _) = s.funding.get_current_rate();
    assert_eq!(long_rate, 105);
}
