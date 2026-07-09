use soroban_sdk::testutils::Address as _;

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
    h.set_price(0, 1_000_0000000);  // Asset 0: $1.00
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
    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &10_0000000_i128,
        &1_u32,
    );

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
        let pos_id = h.pm.open_position(
            &trader,
            &0_u32,
            &Direction::Long,
            &100_0000000_i128,
            &5_u32,
        );
        assert_eq!(pos_id, (i + 1) as u64);

        h.pm.close_position(&trader, &pos_id, &0_u32);
    }

    // All positions closed
    assert_eq!(h.pm.get_trader_position_count(&trader), 0);
    assert_eq!(h.pm.get_open_interest_long(), 0);
    assert_eq!(h.vault.get_locked_liquidity(), 0);

    // Position IDs should have incremented to 6
    let next_pos = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &5_u32,
    );
    assert_eq!(next_pos, 6);
}
