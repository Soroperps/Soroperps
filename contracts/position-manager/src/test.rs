use soroban_sdk::{testutils::Address as _, testutils::Ledger, token, Address, Env};

use crate::PositionManagerContract;
use crate::PositionManagerContractClient;
use perps_oracle_adapter::OracleAdapterContract;
use perps_oracle_adapter::OracleAdapterContractClient;
use perps_types::Direction;
use perps_vault::VaultContract;
use perps_vault::VaultContractClient;

struct TestSetup<'a> {
    env: Env,
    admin: Address,
    usdc_address: Address,
    vault: VaultContractClient<'a>,
    oracle: OracleAdapterContractClient<'a>,
    pm: PositionManagerContractClient<'a>,
    #[allow(dead_code)]
    vault_address: Address,
    #[allow(dead_code)]
    pm_address: Address,
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
        &10_u32,   // 0.1% open fee
        &10_u32,   // 0.1% close fee
        &30_u32,   // max 30x leverage
        &10_0000000_i128, // min 10 USDC collateral
    );

    // Wire vault to trust position manager
    // The PM calls vault functions using vault_address as caller (cross-contract).
    // We need vault to recognize the vault_address itself since the PM uses
    // vault.lock_liquidity(&vault_addr, ...) where vault_addr acts as the caller.
    // Actually, in Soroban cross-contract calls, the caller is the contract making the call.
    // So we need to register the PM's address as the position manager in the vault.
    vault.set_position_manager(&admin, &pm_address);

    // Set timestamp
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    TestSetup {
        env,
        admin,
        usdc_address,
        vault,
        oracle,
        pm,
        vault_address,
        pm_address,
    }
}

fn mint_usdc(env: &Env, usdc_address: &Address, to: &Address, amount: i128) {
    let admin_client = token::StellarAssetClient::new(env, usdc_address);
    admin_client.mint(to, &amount);
}

fn add_liquidity(s: &TestSetup, amount: i128) -> Address {
    let lp = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &lp, amount);
    s.vault.deposit(&lp, &amount);
    lp
}

fn set_price(s: &TestSetup, asset: u32, price: i128) {
    s.oracle.set_price(&s.admin, &asset, &price);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_open_position_long() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000); // 10k USDC LP

    set_price(&s, 0, 1_500_000); // XLM = $0.15

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000); // 200 USDC

    let pos_id = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128, // 100 USDC collateral
        &10_u32,           // 10x leverage
    );

    assert_eq!(pos_id, 1);

    let pos = s.pm.get_position(&trader, &pos_id);
    assert_eq!(pos.collateral, 100_0000000);
    assert_eq!(pos.size, 1000_0000000); // 100 * 10 = 1000 USDC notional
    assert_eq!(pos.leverage, 10);
    assert_eq!(pos.entry_price, 1_500_000);
    assert_eq!(pos.direction, Direction::Long);

    // Check OI updated
    assert_eq!(s.pm.get_open_interest_long(), 1000_0000000);
    assert_eq!(s.pm.get_open_interest_short(), 0);

    // Check trader position count
    assert_eq!(s.pm.get_trader_position_count(&trader), 1);
}

#[test]
fn test_open_position_short() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_500_000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000);

    let pos_id = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Short,
        &100_0000000_i128,
        &5_u32,
    );

    let pos = s.pm.get_position(&trader, &pos_id);
    assert_eq!(pos.direction, Direction::Short);
    assert_eq!(pos.size, 500_0000000); // 100 * 5
    assert_eq!(s.pm.get_open_interest_short(), 500_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_open_leverage_too_high() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_500_000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000);

    // 31x > 30x max
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &31_u32,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #15)")]
fn test_open_insufficient_collateral() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_500_000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000);

    // 5 USDC < 10 USDC minimum
    s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &5_0000000_i128,
        &10_u32,
    );
}

#[test]
fn test_close_long_profit() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_000_0000000); // $1.00

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000);

    let pos_id = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price goes up 10%: $1.00 -> $1.10
    set_price(&s, 0, 1_100_0000000);

    let net_pnl = s.pm.close_position(&trader, &pos_id, &0_u32);

    // PnL = 1000 * (1.10 - 1.00) / 1.00 = 100 USDC
    // Close fee = 1000 * 0.1% = 1 USDC
    // Net = 100 - 1 = 99 USDC
    assert_eq!(net_pnl, 99_0000000);

    // Position should be removed
    assert_eq!(s.pm.get_trader_position_count(&trader), 0);
    assert_eq!(s.pm.get_open_interest_long(), 0);
}

#[test]
fn test_close_long_loss() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_000_0000000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000);

    let pos_id = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price drops 5%: $1.00 -> $0.95
    set_price(&s, 0, 950_0000000);

    let net_pnl = s.pm.close_position(&trader, &pos_id, &0_u32);

    // PnL = 1000 * (0.95 - 1.00) / 1.00 = -50 USDC
    // Close fee = 1000 * 0.1% = 1 USDC
    // Net = -50 - 1 = -51 USDC
    assert_eq!(net_pnl, -51_0000000);
}

#[test]
fn test_close_short_profit() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_000_0000000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000);

    let pos_id = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Short,
        &100_0000000_i128,
        &10_u32,
    );

    // Price drops 10%: $1.00 -> $0.90
    set_price(&s, 0, 900_0000000);

    let net_pnl = s.pm.close_position(&trader, &pos_id, &0_u32);

    // PnL = 1000 * (1.00 - 0.90) / 1.00 = 100 USDC
    // Close fee = 1 USDC
    // Net = 99
    assert_eq!(net_pnl, 99_0000000);
}

#[test]
fn test_close_short_loss() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_000_0000000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000);

    let pos_id = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Short,
        &100_0000000_i128,
        &10_u32,
    );

    // Price goes up 5%: $1.00 -> $1.05
    set_price(&s, 0, 1_050_0000000);

    let net_pnl = s.pm.close_position(&trader, &pos_id, &0_u32);

    // PnL = 1000 * (1.00 - 1.05) / 1.00 = -50
    // Close fee = 1
    // Net = -51
    assert_eq!(net_pnl, -51_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_close_nonexistent_position() {
    let s = setup();
    let trader = Address::generate(&s.env);
    s.pm.close_position(&trader, &999_u64, &0_u32);
}

#[test]
fn test_fee_calculation() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_000_0000000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 500_0000000);

    // Open: 100 USDC * 10x = 1000 USDC notional
    // Open fee: 1000 * 10 / 10000 = 1 USDC
    // Total USDC transferred: 100 + 1 = 101
    let _pos_id = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Trader should have 500 - 101 = 399 USDC left
    let token_client = token::TokenClient::new(&s.env, &s.usdc_address);
    let trader_balance = token_client.balance(&trader);
    assert_eq!(trader_balance, 399_0000000);
}

#[test]
fn test_multiple_positions() {
    let s = setup();
    add_liquidity(&s, 50_000_0000000);
    set_price(&s, 0, 1_000_0000000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 1000_0000000);

    let pos1 = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );
    let pos2 = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Short,
        &100_0000000_i128,
        &5_u32,
    );

    assert_eq!(pos1, 1);
    assert_eq!(pos2, 2);
    assert_eq!(s.pm.get_trader_position_count(&trader), 2);
    assert_eq!(s.pm.get_open_interest_long(), 1000_0000000);
    assert_eq!(s.pm.get_open_interest_short(), 500_0000000);

    // Close first position
    set_price(&s, 0, 1_000_0000000); // same price, only fees
    s.pm.close_position(&trader, &pos1, &0_u32);

    assert_eq!(s.pm.get_trader_position_count(&trader), 1);
    assert_eq!(s.pm.get_open_interest_long(), 0);
    assert_eq!(s.pm.get_open_interest_short(), 500_0000000);
}

#[test]
fn test_unrealized_pnl() {
    let s = setup();
    add_liquidity(&s, 10_000_0000000);
    set_price(&s, 0, 1_000_0000000);

    let trader = Address::generate(&s.env);
    mint_usdc(&s.env, &s.usdc_address, &trader, 200_0000000);

    let pos_id = s.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price up 20%
    set_price(&s, 0, 1_200_0000000);

    let pnl = s.pm.get_unrealized_pnl(&trader, &pos_id, &0_u32);
    // 1000 * (1.20 - 1.00) / 1.00 = 200 USDC
    assert_eq!(pnl, 200_0000000);
}
