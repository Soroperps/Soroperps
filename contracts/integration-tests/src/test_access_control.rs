use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

use perps_types::Direction;

use crate::harness::TestHarness;

// ---------------------------------------------------------------------------
// Access control tests — verify auth guards across contracts
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_vault_lock_unauthorized() {
    let h = TestHarness::setup();

    // Random address tries to lock vault liquidity — should fail
    let attacker = Address::generate(&h.env);
    h.vault.lock_liquidity(&attacker, &1000_0000000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_vault_unlock_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    h.vault.unlock_liquidity(&attacker, &1000_0000000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_vault_settle_pnl_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    let trader = Address::generate(&h.env);
    h.vault.settle_pnl(&attacker, &trader, &100_0000000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_vault_collect_fee_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    h.vault.collect_fee(&attacker, &100_0000000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_vault_set_position_manager_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    let fake_pm = Address::generate(&h.env);
    h.vault.set_position_manager(&attacker, &fake_pm);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_oracle_set_price_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    h.oracle.set_price(&attacker, &0_u32, &1_000_0000000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_pm_set_fee_rates_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    h.pm.set_fee_rates(&attacker, &50_u32, &50_u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_pm_set_max_leverage_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    h.pm.set_max_leverage(&attacker, &50_u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_le_set_maintenance_margin_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    h.le.set_maintenance_margin(&attacker, &1000_u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_funding_set_interval_unauthorized() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    h.funding.set_funding_interval(&attacker, &7200_u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_vault_double_initialize() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    let fake_token = Address::generate(&h.env);
    h.vault.initialize(&attacker, &fake_token, &8000_u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_pm_double_initialize() {
    let h = TestHarness::setup();

    let attacker = Address::generate(&h.env);
    let fake = Address::generate(&h.env);
    h.pm.initialize(
        &attacker, &fake, &fake, &fake,
        &10_u32, &10_u32, &30_u32, &10_0000000_i128,
    );
}

#[test]
fn test_admin_can_update_config() {
    let h = TestHarness::setup();

    // Admin updates fee rates
    h.pm.set_fee_rates(&h.admin, &20_u32, &20_u32);

    // Admin updates max leverage
    h.pm.set_max_leverage(&h.admin, &50_u32);

    // Admin updates vault max utilization
    h.vault.set_max_utilization(&h.admin, &9000_u32);

    // Admin updates oracle staleness
    h.oracle.set_staleness_threshold(&h.admin, &600_u64);

    // Admin updates liquidation engine params
    h.le.set_maintenance_margin(&h.admin, &800_u32);
    h.le.set_liquidation_penalty(&h.admin, &300_u32);
    h.le.set_keeper_reward(&h.admin, &100_u32);

    // Admin updates funding rate params
    h.funding.set_funding_interval(&h.admin, &7200_u64);
    h.funding.set_max_rate(&h.admin, &20_u32);

    // Verify new leverage works
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);
    let trader = h.create_trader(500_0000000);

    // 50x leverage should now work (was 30x max before)
    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &10_0000000_i128,
        &50_u32,
    );
    let pos = h.pm.get_position(&trader, &pos_id);
    assert_eq!(pos.size, 500_0000000); // 10 * 50
}

#[test]
#[should_panic]
fn test_pm_liquidate_position_unauthorized_caller() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);
    h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Random address tries to call liquidate_position directly on PM
    // (only liquidation engine should be allowed)
    let attacker = Address::generate(&h.env);
    h.pm.liquidate_position(&attacker, &trader, &1_u64, &0_u32);
}
