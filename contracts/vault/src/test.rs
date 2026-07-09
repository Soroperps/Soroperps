use soroban_sdk::{testutils::Address as _, token, Address, Env};

use crate::VaultContract;
use crate::VaultContractClient;

fn setup() -> (Env, VaultContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let usdc_admin = Address::generate(&env);
    let usdc_contract = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_address = usdc_contract.address();

    let vault_id = env.register(VaultContract, ());
    let vault = VaultContractClient::new(&env, &vault_id);

    vault.initialize(&admin, &usdc_address, &8000_u32);

    (env, vault, admin, usdc_address, usdc_admin)
}

fn mint_usdc(env: &Env, usdc_address: &Address, _usdc_admin: &Address, to: &Address, amount: i128) {
    let admin_client = token::StellarAssetClient::new(env, usdc_address);
    admin_client.mint(to, &amount);
}

fn usdc_balance(env: &Env, usdc_address: &Address, account: &Address) -> i128 {
    let client = token::TokenClient::new(env, usdc_address);
    client.balance(account)
}

// ---------------------------------------------------------------------------
// Initialization tests
// ---------------------------------------------------------------------------

#[test]
fn test_initialize() {
    let (_env, vault, _admin, _usdc_address, _) = setup();
    assert_eq!(vault.get_total_shares(), 0);
    assert_eq!(vault.get_total_deposits(), 0);
    assert_eq!(vault.get_locked_liquidity(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_initialize_twice_fails() {
    let (_env, vault, admin, usdc_address, _) = setup();
    vault.initialize(&admin, &usdc_address, &8000_u32);
}

// ---------------------------------------------------------------------------
// Deposit tests
// ---------------------------------------------------------------------------

#[test]
fn test_deposit_first_lp() {
    let (env, vault, _, usdc_address, usdc_admin) = setup();
    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);

    let shares = vault.deposit(&lp, &1000_0000000_i128);

    // First deposit: 1:1 ratio
    assert_eq!(shares, 1000_0000000);
    assert_eq!(vault.get_total_shares(), 1000_0000000);
    assert_eq!(vault.get_total_deposits(), 1000_0000000);
    assert_eq!(vault.get_share_balance(&lp), 1000_0000000);
    assert_eq!(usdc_balance(&env, &usdc_address, &lp), 0);
}

#[test]
fn test_deposit_second_lp() {
    let (env, vault, _, usdc_address, usdc_admin) = setup();

    let lp1 = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp1, 1000_0000000);
    vault.deposit(&lp1, &1000_0000000_i128);

    let lp2 = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp2, 500_0000000);
    let shares = vault.deposit(&lp2, &500_0000000_i128);

    // 500 * 1000 / 1000 = 500 shares
    assert_eq!(shares, 500_0000000);
    assert_eq!(vault.get_total_shares(), 1500_0000000);
    assert_eq!(vault.get_total_deposits(), 1500_0000000);
    assert_eq!(vault.get_share_balance(&lp2), 500_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_deposit_zero_amount() {
    let (env, vault, _, _, _) = setup();
    let lp = Address::generate(&env);
    vault.deposit(&lp, &0_i128);
}

// ---------------------------------------------------------------------------
// Withdraw tests
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_full() {
    let (env, vault, _, usdc_address, usdc_admin) = setup();
    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);

    vault.deposit(&lp, &1000_0000000_i128);
    let returned = vault.withdraw(&lp, &1000_0000000_i128);

    assert_eq!(returned, 1000_0000000);
    assert_eq!(vault.get_total_shares(), 0);
    assert_eq!(vault.get_total_deposits(), 0);
    assert_eq!(vault.get_share_balance(&lp), 0);
    assert_eq!(usdc_balance(&env, &usdc_address, &lp), 1000_0000000);
}

#[test]
fn test_withdraw_partial() {
    let (env, vault, _, usdc_address, usdc_admin) = setup();
    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);

    vault.deposit(&lp, &1000_0000000_i128);
    let returned = vault.withdraw(&lp, &400_0000000_i128);

    assert_eq!(returned, 400_0000000);
    assert_eq!(vault.get_total_shares(), 600_0000000);
    assert_eq!(vault.get_total_deposits(), 600_0000000);
    assert_eq!(vault.get_share_balance(&lp), 600_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_withdraw_exceeds_balance() {
    let (env, vault, _, usdc_address, usdc_admin) = setup();
    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);

    vault.deposit(&lp, &1000_0000000_i128);
    vault.withdraw(&lp, &1500_0000000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_withdraw_exceeds_utilization() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();

    // Setup position manager
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    // Lock 500 USDC (50% utilization)
    vault.lock_liquidity(&pm, &500_0000000_i128);

    // Try to withdraw 800 — would leave 200 deposits with 500 locked = 250% utilization
    vault.withdraw(&lp, &800_0000000_i128);
}

// ---------------------------------------------------------------------------
// Liquidity management tests
// ---------------------------------------------------------------------------

#[test]
fn test_lock_unlock_liquidity() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    vault.lock_liquidity(&pm, &300_0000000_i128);
    assert_eq!(vault.get_locked_liquidity(), 300_0000000);
    assert_eq!(vault.get_available_liquidity(), 700_0000000);

    // Utilization: 300/1000 = 30% = 3000 bps
    assert_eq!(vault.get_utilization(), 3000);

    vault.unlock_liquidity(&pm, &300_0000000_i128);
    assert_eq!(vault.get_locked_liquidity(), 0);
    assert_eq!(vault.get_available_liquidity(), 1000_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_lock_exceeds_max_utilization() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    // Try to lock 900 USDC (90% > 80% max utilization)
    vault.lock_liquidity(&pm, &900_0000000_i128);
}

// ---------------------------------------------------------------------------
// PnL settlement tests
// ---------------------------------------------------------------------------

#[test]
fn test_settle_positive_pnl() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    let trader = Address::generate(&env);

    // Trader wins 100 USDC — vault pays out
    vault.settle_pnl(&pm, &trader, &100_0000000_i128);

    assert_eq!(vault.get_total_deposits(), 900_0000000);
    assert_eq!(usdc_balance(&env, &usdc_address, &trader), 100_0000000);
}

#[test]
fn test_settle_negative_pnl() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    let trader = Address::generate(&env);

    // Trader loses 100 USDC — vault gains (USDC already transferred by PM)
    vault.settle_pnl(&pm, &trader, &-100_0000000_i128);

    assert_eq!(vault.get_total_deposits(), 1100_0000000);
}

// ---------------------------------------------------------------------------
// Auth tests
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_unauthorized_lock() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    // Random address tries to lock liquidity
    let rando = Address::generate(&env);
    vault.lock_liquidity(&rando, &100_0000000_i128);
}

// ---------------------------------------------------------------------------
// Share price tests
// ---------------------------------------------------------------------------

#[test]
fn test_share_price_after_trader_loss() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    // Initial share price: 1.0 (10_000_000 with 7 decimals)
    assert_eq!(vault.get_share_price(), 10_000_000);

    // Trader loses 200 USDC → pool grows
    vault.settle_pnl(&pm, &Address::generate(&env), &-200_0000000_i128);

    // Share price: 1200/1000 = 1.2 = 12_000_000
    assert_eq!(vault.get_share_price(), 12_000_000);
}

#[test]
fn test_share_price_after_trader_win() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    let trader = Address::generate(&env);

    // Trader wins 200 USDC → pool shrinks
    vault.settle_pnl(&pm, &trader, &200_0000000_i128);

    // Share price: 800/1000 = 0.8 = 8_000_000
    assert_eq!(vault.get_share_price(), 8_000_000);
}

#[test]
fn test_fee_collection() {
    let (env, vault, admin, usdc_address, usdc_admin) = setup();
    let pm = Address::generate(&env);
    vault.set_position_manager(&admin, &pm);

    let lp = Address::generate(&env);
    mint_usdc(&env, &usdc_address, &usdc_admin, &lp, 1000_0000000);
    vault.deposit(&lp, &1000_0000000_i128);

    // Collect 10 USDC fee
    vault.collect_fee(&pm, &10_0000000_i128);

    // Total deposits increased (fees accrue to LPs)
    assert_eq!(vault.get_total_deposits(), 1010_0000000);
    // Share price: 1010/1000 = 1.01
    assert_eq!(vault.get_share_price(), 10_100_000);
}
