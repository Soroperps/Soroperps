use soroban_sdk::{testutils::Address as _, testutils::Ledger, token, Address, Env};

use crate::LiquidationEngineContract;
use crate::LiquidationEngineContractClient;
use perps_oracle_adapter::OracleAdapterContract;
use perps_oracle_adapter::OracleAdapterContractClient;
use perps_position_manager::PositionManagerContract;
use perps_position_manager::PositionManagerContractClient;
use perps_types::Direction;
use perps_vault::VaultContract;
use perps_vault::VaultContractClient;

struct TestSetup<'a> {
    env: Env,
    admin: Address,
    usdc_address: Address,
    #[allow(dead_code)]
    vault: VaultContractClient<'a>,
    oracle: OracleAdapterContractClient<'a>,
    pm: PositionManagerContractClient<'a>,
    le: LiquidationEngineContractClient<'a>,
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
        &10_u32,          // 0.1% open fee
        &10_u32,          // 0.1% close fee
        &30_u32,          // max 30x leverage
        &10_0000000_i128, // min 10 USDC collateral
    );

    // Deploy liquidation engine
    let le_address = env.register(LiquidationEngineContract, ());
    let le = LiquidationEngineContractClient::new(&env, &le_address);
    le.initialize(
        &admin,
        &pm_address,
        &oracle_address,
        &500_u32, // 5% maintenance margin
        &250_u32, // 2.5% liquidation penalty
        &50_u32,  // 0.5% keeper reward
    );

    // Wire contracts
    vault.set_position_manager(&admin, &pm_address);
    pm.set_liquidation_engine(&admin, &le_address);

    // Set timestamp
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    // Add LP liquidity
    let lp = Address::generate(&env);
    let lp_admin = token::StellarAssetClient::new(&env, &usdc_address);
    lp_admin.mint(&lp, &100_000_0000000_i128);
    vault.deposit(&lp, &100_000_0000000_i128);

    TestSetup {
        env,
        admin,
        usdc_address,
        vault,
        oracle,
        pm,
        le,
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
fn test_healthy_position_not_liquidatable() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price unchanged — margin ratio = 100/1000 = 10% = 1000 bps > 500 bps
    let is_liq = s.le.is_liquidatable(&trader, &1_u64, &0_u32);
    assert!(!is_liq);
}

#[test]
fn test_margin_ratio_calculation() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price unchanged: margin = 100, size = 1000
    // margin_ratio = 100 * 10000 / 1000 = 1000 bps = 10%
    let ratio = s.le.get_margin_ratio(&trader, &1_u64, &0_u32);
    assert_eq!(ratio, 1000);

    // Price drops 5%: PnL = 1000 * (0.95 - 1.00) / 1.00 = -50
    // effective_margin = 100 - 50 = 50
    // margin_ratio = 50 * 10000 / 1000 = 500 bps = 5% (exactly at maintenance)
    s.oracle.set_price(&s.admin, &0_u32, &950_0000000_i128);
    let ratio = s.le.get_margin_ratio(&trader, &1_u64, &0_u32);
    assert_eq!(ratio, 500);
}

#[test]
fn test_underwater_position_liquidatable() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price drops 6%: PnL = 1000 * (0.94 - 1.00) / 1.00 = -60
    // effective_margin = 100 - 60 = 40
    // margin_ratio = 40 * 10000 / 1000 = 400 bps < 500 bps maintenance
    s.oracle.set_price(&s.admin, &0_u32, &940_0000000_i128);
    let is_liq = s.le.is_liquidatable(&trader, &1_u64, &0_u32);
    assert!(is_liq);
}

#[test]
fn test_liquidate_long_position() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price drops 8%: PnL = 1000 * (0.92 - 1.00) / 1.00 = -80
    // effective_margin = 100 - 80 = 20
    // margin_ratio = 20 * 10000 / 1000 = 200 bps < 500 bps
    s.oracle.set_price(&s.admin, &0_u32, &920_0000000_i128);

    let keeper = Address::generate(&s.env);
    s.le.liquidate(&keeper, &trader, &1_u64, &0_u32);

    // Position should be closed
    assert_eq!(s.pm.get_trader_position_count(&trader), 0);
    assert_eq!(s.pm.get_open_interest_long(), 0);
}

#[test]
fn test_liquidate_short_position() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Short,
        &100_0000000_i128,
        &10_u32,
    );

    // Price rises 8%: PnL = 1000 * (1.00 - 1.08) / 1.00 = -80
    // effective_margin = 100 - 80 = 20
    // margin_ratio = 200 bps < 500 bps
    s.oracle.set_price(&s.admin, &0_u32, &1_080_0000000_i128);

    let keeper = Address::generate(&s.env);
    s.le.liquidate(&keeper, &trader, &1_u64, &0_u32);

    assert_eq!(s.pm.get_trader_position_count(&trader), 0);
    assert_eq!(s.pm.get_open_interest_short(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_liquidate_healthy_position_fails() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price unchanged — position is healthy
    let keeper = Address::generate(&s.env);
    s.le.liquidate(&keeper, &trader, &1_u64, &0_u32);
}

#[test]
fn test_margin_ratio_with_profit() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price rises 10%: PnL = 1000 * (1.10 - 1.00) / 1.00 = 100
    // effective_margin = 100 + 100 = 200
    // margin_ratio = 200 * 10000 / 1000 = 2000 bps = 20%
    s.oracle.set_price(&s.admin, &0_u32, &1_100_0000000_i128);
    let ratio = s.le.get_margin_ratio(&trader, &1_u64, &0_u32);
    assert_eq!(ratio, 2000);
    assert!(!s.le.is_liquidatable(&trader, &1_u64, &0_u32));
}

#[test]
fn test_deeply_underwater_position() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price drops 15%: PnL = 1000 * (0.85 - 1.00) / 1.00 = -150
    // effective_margin = 100 - 150 = -50 (negative!)
    // margin_ratio = -50 * 10000 / 1000 = -500 bps
    s.oracle.set_price(&s.admin, &0_u32, &850_0000000_i128);
    let ratio = s.le.get_margin_ratio(&trader, &1_u64, &0_u32);
    assert_eq!(ratio, -500);
    assert!(s.le.is_liquidatable(&trader, &1_u64, &0_u32));

    // Can still liquidate (even though underwater)
    let keeper = Address::generate(&s.env);
    s.le.liquidate(&keeper, &trader, &1_u64, &0_u32);
    assert_eq!(s.pm.get_trader_position_count(&trader), 0);
}

#[test]
fn test_boundary_maintenance_margin() {
    let s = setup();
    s.oracle.set_price(&s.admin, &0_u32, &1_000_0000000_i128);

    let trader = create_trader(&s, 500_0000000);
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Exactly at 5% maintenance: PnL = -50, margin = 50, ratio = 500 bps
    // margin_ratio == maintenance, NOT liquidatable (must be strictly below)
    s.oracle.set_price(&s.admin, &0_u32, &950_0000000_i128);
    assert!(!s.le.is_liquidatable(&trader, &1_u64, &0_u32));

    // Just below: price drops slightly more
    // PnL = 1000 * (0.949 - 1.00) / 1.00 = -51
    // margin = 100 - 51 = 49, ratio = 490 < 500
    s.oracle.set_price(&s.admin, &0_u32, &949_0000000_i128);
    assert!(s.le.is_liquidatable(&trader, &1_u64, &0_u32));
}
