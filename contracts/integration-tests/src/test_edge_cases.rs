use soroban_sdk::{testutils::Address as _, Address};

use perps_types::Direction;

use crate::harness::TestHarness;

// ---------------------------------------------------------------------------
// Edge case and adversarial tests
// ---------------------------------------------------------------------------

#[test]
fn test_open_position_zero_lp_liquidity() {
    let h = TestHarness::setup();
    // No LP deposits — zero liquidity in the vault
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    // Trader can still open (their collateral goes to vault, utilization check passes)
    // but if price moves against them, there's no LP capital to pay them.
    // This is a known edge case — in production, a minimum vault balance would be enforced.
    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );
    assert_eq!(pos_id, 1);

    // Close at same price — works because payout comes from trader's own collateral
    let pnl = h.pm.close_position(&trader, &pos_id, &0_u32);
    assert_eq!(pnl, -1_0000000); // Only close fee lost
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_max_leverage_boundary() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    // 30x is max, 31x should fail
    h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &31_u32,
    );
}

#[test]
fn test_max_leverage_exactly_at_limit() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    // 30x exactly should work
    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &10_0000000_i128, // min collateral
        &30_u32,
    );
    assert_eq!(pos_id, 1);

    let pos = h.pm.get_position(&trader, &pos_id);
    assert_eq!(pos.size, 300_0000000); // 10 * 30
}

#[test]
fn test_near_total_loss() {
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

    // Price drops 9.5% — position nearly wiped out but technically still above 0
    // PnL = 1000 * (0.905 - 1.00) / 1.00 = -95
    // This is liquidatable (margin_ratio = (100-95)*10000/1000 = 50 bps < 500 bps)
    h.set_price(0, 905_0000000);

    assert!(h.le.is_liquidatable(&trader, &1_u64, &0_u32));

    // Liquidate
    let keeper = soroban_sdk::Address::generate(&h.env);
    h.le.liquidate(&keeper, &trader, &1_u64, &0_u32);

    assert_eq!(h.pm.get_trader_position_count(&trader), 0);
}

#[test]
fn test_deeply_underwater_liquidation() {
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

    // Price drops 15% — position deeply underwater
    // PnL = 1000 * -0.15 = -150, effective margin = 100 - 150 = -50
    h.set_price(0, 850_0000000);

    let ratio = h.le.get_margin_ratio(&trader, &1_u64, &0_u32);
    assert_eq!(ratio, -500); // Negative margin ratio

    // Can still liquidate
    let keeper = soroban_sdk::Address::generate(&h.env);
    h.le.liquidate(&keeper, &trader, &1_u64, &0_u32);
    assert_eq!(h.pm.get_trader_position_count(&trader), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_double_liquidation_fails() {
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

    h.set_price(0, 900_0000000);

    let keeper = soroban_sdk::Address::generate(&h.env);
    h.le.liquidate(&keeper, &trader, &1_u64, &0_u32);

    // Second liquidation fails with PositionNotFound (#7) — position was already removed
    h.le.liquidate(&keeper, &trader, &1_u64, &0_u32);
}

#[test]
fn test_multiple_assets() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);

    // Set prices for two different assets
    h.set_price(0, 1_000_0000000); // Asset 0: $1.00
    h.set_price(1, 50_000_0000000); // Asset 1: $50.00

    let trader = h.create_trader(1000_0000000);

    // Open positions on different assets
    let pos1 = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );
    let pos2 = h.pm.open_position(
        &trader,
        &1_u32,
        &Direction::Short,
        &100_0000000_i128,
        &5_u32,
    );

    // Move asset 0 up 10%, asset 1 down 10%
    h.set_price(0, 1_100_0000000);
    h.set_price(1, 45_000_0000000);

    // Both should be profitable
    let pnl1 = h.pm.close_position(&trader, &pos1, &0_u32);
    let pnl2 = h.pm.close_position(&trader, &pos2, &1_u32);

    assert!(pnl1 > 0); // Long profited from price increase
    assert!(pnl2 > 0); // Short profited from price decrease
}

#[test]
fn test_open_close_same_price() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Close at same price — only fees are lost
    let pnl = h.pm.close_position(&trader, &pos_id, &0_u32);
    // PnL = 0, close fee = 1. Net = -1
    assert_eq!(pnl, -1_0000000);

    // Trader started with 500, paid 101 (collateral+open_fee), got back 99 (collateral - close_fee)
    // 500 - 101 + 99 = 498
    assert_eq!(h.usdc_balance(&trader), 498_0000000);
}

#[test]
fn test_small_position_minimum_collateral() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    // Minimum collateral (10 USDC) with minimum leverage (1x)
    let pos_id =
        h.pm.open_position(&trader, &0_u32, &Direction::Long, &10_0000000_i128, &1_u32);

    let pos = h.pm.get_position(&trader, &pos_id);
    assert_eq!(pos.size, 10_0000000); // 10 * 1x
    assert_eq!(pos.collateral, 10_0000000);
}

#[test]
fn test_funding_balanced_oi() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader1 = h.create_trader(500_0000000);
    let trader2 = h.create_trader(500_0000000);

    // Equal OI: 1000 long, 1000 short
    h.pm.open_position(
        &trader1,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );
    h.pm.open_position(
        &trader2,
        &0_u32,
        &Direction::Short,
        &100_0000000_i128,
        &10_u32,
    );

    // Balanced OI — base rate should be 0, only spread applies
    let (long_rate, short_rate) = h.funding.get_current_rate();
    // skew = 0, base_rate = 0
    // long_rate = 0 + 5 = 5 (spread only)
    // short_rate = -(0 - 5) = 5 (spread only)
    assert_eq!(long_rate, 5);
    assert_eq!(short_rate, 5);
}

#[test]
fn test_sequential_open_close_positions() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(2000_0000000);

    // Open and close 5 positions sequentially
    for i in 0..5 {
        let pos_id =
            h.pm.open_position(&trader, &0_u32, &Direction::Long, &100_0000000_i128, &5_u32);
        assert_eq!(pos_id, (i + 1) as u64);

        h.pm.close_position(&trader, &pos_id, &0_u32);
    }

    // All positions closed
    assert_eq!(h.pm.get_trader_position_count(&trader), 0);
    assert_eq!(h.pm.get_open_interest_long(), 0);
    assert_eq!(h.vault.get_locked_liquidity(), 0);

    // Position IDs should have incremented to 6
    let next_pos =
        h.pm.open_position(&trader, &0_u32, &Direction::Long, &100_0000000_i128, &5_u32);
    assert_eq!(next_pos, 6);
}

// ---------------------------------------------------------------------------
// Edge case fixes — regression tests for audit findings
// ---------------------------------------------------------------------------

/// #20: Close position with wrong asset should fail (AssetMismatch).
#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_close_position_wrong_asset() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);
    h.set_price(1, 50_000_0000000);

    let trader = h.create_trader(500_0000000);

    // Open on asset 0
    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Try to close with asset 1 — should fail with AssetMismatch (#17)
    h.pm.close_position(&trader, &pos_id, &1_u32);
}

/// #3: PnL capped at collateral — vault accounting stays consistent
/// when loss exceeds collateral (>100% loss scenario).
#[test]
fn test_loss_capped_at_collateral() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    // 100 USDC at 10x = 1000 notional long
    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price drops 50%: raw PnL = 1000 * -0.50 = -500, but collateral = 100
    // PnL should be capped at -100
    h.set_price(0, 500_0000000);

    let pnl = h.pm.close_position(&trader, &pos_id, &0_u32);
    // Capped PnL = -100, fee = 1, net = -101
    assert_eq!(pnl, -101_0000000);

    // Trader payout: collateral + capped_pnl - fee = 100 + (-100) - 1 = -1 → 0 (no payout)
    // Vault total_deposits should NOT be inflated beyond actual balance
    let vault_deposits = h.vault.get_total_deposits();
    let vault_locked = h.vault.get_locked_liquidity();

    // After open: total_deposits = 100000 + 1(open_fee via collect_fee) = 100001
    // After close: new_deposits = 100001 - (-100)(capped PnL) + 1(close_fee) = 100102
    assert_eq!(vault_deposits, 100_102_0000000);
    assert_eq!(vault_locked, 0);
}

/// #10: Fee rates cannot be set beyond 10% (1000 bps) post-init.
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_set_fee_rates_too_high() {
    let h = TestHarness::setup();

    // Try to set 100% open fee — should fail with InvalidConfig
    h.pm.set_fee_rates(&h.admin, &10000_u32, &10_u32);
}

/// #18: High-leverage position born at low margin ratio.
/// With 30x leverage, initial margin = collateral/size = 1/30 = 333 bps < 500 bps maintenance.
/// This means the position is immediately liquidatable.
#[test]
fn test_high_leverage_immediately_liquidatable() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    // Open at 30x leverage
    h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &30_u32,
    );

    // Margin ratio at opening = 100 * 10000 / 3000 = 333 bps < 500 bps maintenance
    let ratio = h.le.get_margin_ratio(&trader, &1_u64, &0_u32);
    assert_eq!(ratio, 333);

    // Position is immediately liquidatable — this is a known risk of max leverage
    let is_liq = h.le.is_liquidatable(&trader, &1_u64, &0_u32);
    assert!(is_liq);
}

/// #8: Share price inflation — first deposit is tiny, vault profits, second depositor blocked.
#[test]
fn test_share_price_inflation_small_first_deposit() {
    let h = TestHarness::setup();

    // First LP deposits 100 USDC (small but enough for a position)
    let _lp1 = h.add_liquidity(100_0000000);
    let initial_shares = h.vault.get_total_shares();
    assert_eq!(initial_shares, 100_0000000);

    // Simulate vault profit: trader opens and closes at a loss
    h.set_price(0, 1_000_0000000);
    let trader = h.create_trader(200_0000000);
    h.pm.open_position(&trader, &0_u32, &Direction::Long, &10_0000000_i128, &5_u32);
    h.set_price(0, 900_0000000); // -10% drop
    h.pm.close_position(&trader, &1_u64, &0_u32);

    // Vault profited from trader loss — share price should be > 1.0
    let share_price = h.vault.get_share_price();
    assert!(share_price > 10_000_000); // > 1.0 (7 decimals)

    // Second LP deposits 1 USDC — should still get some shares
    let lp2 = h.add_liquidity(1_0000000);
    let lp2_shares = h.vault.get_share_balance(&lp2);
    assert!(lp2_shares > 0);
}

/// Position stores asset — verify it's correctly recorded.
#[test]
fn test_position_stores_asset() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);
    h.set_price(1, 50_000_0000000);

    let trader = h.create_trader(1000_0000000);

    let pos0 =
        h.pm.open_position(&trader, &0_u32, &Direction::Long, &100_0000000_i128, &5_u32);
    let pos1 = h.pm.open_position(
        &trader,
        &1_u32,
        &Direction::Short,
        &100_0000000_i128,
        &5_u32,
    );

    let p0 = h.pm.get_position(&trader, &pos0);
    let p1 = h.pm.get_position(&trader, &pos1);

    assert_eq!(p0.asset, 0);
    assert_eq!(p1.asset, 1);

    // Can close with correct asset
    h.pm.close_position(&trader, &pos0, &0_u32);
    h.pm.close_position(&trader, &pos1, &1_u32);
}

/// Vault settlement guards — locked liquidity can't go negative.
#[test]
fn test_vault_deposits_stay_consistent_after_trades() {
    let h = TestHarness::setup();
    let lp = h.add_liquidity(10_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    // Open and close several positions, verify vault is always consistent
    for _ in 0..3 {
        h.pm.open_position(&trader, &0_u32, &Direction::Long, &50_0000000_i128, &5_u32);
    }

    // Close all at small profit
    h.set_price(0, 1_010_0000000);
    for i in 1..=3 {
        h.pm.close_position(&trader, &(i as u64), &0_u32);
    }

    // Vault invariants
    let locked = h.vault.get_locked_liquidity();
    let deposits = h.vault.get_total_deposits();
    assert_eq!(locked, 0);
    assert!(deposits > 0);

    // LP can withdraw everything
    let shares = h.vault.get_share_balance(&lp);
    let withdrawn = h.vault.withdraw(&lp, &shares);
    assert!(withdrawn > 0);
}

/// Liquidation with asset mismatch should fail.
#[test]
#[should_panic]
fn test_liquidate_wrong_asset() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);
    h.set_price(1, 50_000_0000000);

    let trader = h.create_trader(500_0000000);

    // Open on asset 0
    h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Make asset 1 crash (shouldn't affect asset 0 position)
    h.set_price(1, 1_0000000);

    // Try to liquidate with asset 1 — should fail (asset mismatch in PM)
    let keeper = Address::generate(&h.env);
    h.le.liquidate(&keeper, &trader, &1_u64, &1_u32);
}
