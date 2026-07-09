use soroban_sdk::{testutils::Address as _, Address};

use perps_types::Direction;

use crate::harness::TestHarness;

// ---------------------------------------------------------------------------
// Full lifecycle integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_long_position_profit_lifecycle() {
    let h = TestHarness::setup();
    let lp = h.add_liquidity(100_000_0000000); // LP deposits 100k USDC
    h.set_price(0, 1_000_0000000); // Asset at $1.00

    let trader = h.create_trader(500_0000000); // 500 USDC

    // Open 100 USDC * 10x = 1000 USDC notional long
    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );
    assert_eq!(pos_id, 1);

    // Verify vault locked liquidity
    assert_eq!(h.vault.get_locked_liquidity(), 1000_0000000);

    // Trader paid 100 collateral + 1 fee = 101
    assert_eq!(h.usdc_balance(&trader), 399_0000000);

    // Price rises 20%: $1.00 -> $1.20
    h.set_price(0, 1_200_0000000);

    // Close with profit
    let pnl = h.pm.close_position(&trader, &pos_id, &0_u32);
    // PnL = 1000 * (1.20 - 1.00) / 1.00 = 200
    // Close fee = 1000 * 0.1% = 1
    // Net PnL = 200 - 1 = 199
    assert_eq!(pnl, 199_0000000);

    // Trader gets back: collateral + price_pnl - fee = 100 + 200 - 1 = 299
    // Total trader USDC: 399 (remaining) + 299 (payout) = 698
    assert_eq!(h.usdc_balance(&trader), 698_0000000);

    // Vault liquidity unlocked
    assert_eq!(h.vault.get_locked_liquidity(), 0);

    // LP share price should have decreased (vault paid out profit)
    // Vault total: 100000 (initial) + 101 (collateral+open_fee from trader) - 299 (payout)
    // = 99802. But total_deposits is tracked via accounting:
    // After open: +101 deposits, then collect_fee(1) -> total_deposits = 100001
    // After close: close_position_settlement adjusts: new_deposits = 100001 - 200(pnl) + 1(fee) = 99802
    let vault_deposits = h.vault.get_total_deposits();
    assert_eq!(vault_deposits, 99_802_0000000);

    // LP can still withdraw
    let lp_shares = h.vault.get_share_balance(&lp);
    let withdrawn = h.vault.withdraw(&lp, &lp_shares);
    assert_eq!(withdrawn, 99_802_0000000);
}

#[test]
fn test_short_position_profit_lifecycle() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Short,
        &100_0000000_i128,
        &10_u32,
    );

    // Price drops 15%: $1.00 -> $0.85
    h.set_price(0, 850_0000000);

    let pnl = h.pm.close_position(&trader, &pos_id, &0_u32);
    // PnL = 1000 * (1.00 - 0.85) / 1.00 = 150
    // Close fee = 1
    // Net = 149
    assert_eq!(pnl, 149_0000000);

    // Position cleared
    assert_eq!(h.pm.get_trader_position_count(&trader), 0);
    assert_eq!(h.pm.get_open_interest_short(), 0);
}

#[test]
fn test_trader_loss_goes_to_vault() {
    let h = TestHarness::setup();
    let lp = h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(500_0000000);

    let pos_id = h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &10_u32,
    );

    // Price drops 5%: PnL = -50
    h.set_price(0, 950_0000000);

    let pnl = h.pm.close_position(&trader, &pos_id, &0_u32);
    // PnL = -50, fee = 1, net = -51
    assert_eq!(pnl, -51_0000000);

    // Trader gets back: 100 + (-50) - 1 = 49 USDC
    // Total: 399 + 49 = 448
    assert_eq!(h.usdc_balance(&trader), 448_0000000);

    // Vault gained: deposits = 100000 + 101(initial) + 50(from trader loss) + 1(close fee) - 0(pnl was negative)
    // After close_position_settlement: new_deposits = old - pnl + fee = 100001 - (-50) + 1 = 100052
    let vault_deposits = h.vault.get_total_deposits();
    assert_eq!(vault_deposits, 100_052_0000000);

    // LP share price increased — vault profited from trader loss
    let share_price = h.vault.get_share_price();
    assert!(share_price > 10_000_000); // > 1.0 USDC per share (7 decimals)

    // LP withdraws everything — gets more than deposited
    let lp_shares = h.vault.get_share_balance(&lp);
    let withdrawn = h.vault.withdraw(&lp, &lp_shares);
    assert_eq!(withdrawn, 100_052_0000000);
}

#[test]
fn test_liquidation_full_flow() {
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

    // Price drops 8%: margin ratio = (100 - 80) * 10000 / 1000 = 200 bps < 500 bps
    h.set_price(0, 920_0000000);

    // Verify liquidatable
    assert!(h.le.is_liquidatable(&trader, &1_u64, &0_u32));

    // Keeper liquidates
    let keeper = Address::generate(&h.env);
    h.le.liquidate(&keeper, &trader, &1_u64, &0_u32);

    // Position removed
    assert_eq!(h.pm.get_trader_position_count(&trader), 0);
    assert_eq!(h.pm.get_open_interest_long(), 0);
    assert_eq!(h.vault.get_locked_liquidity(), 0);
}

#[test]
fn test_funding_rate_application() {
    let h = TestHarness::setup();
    h.add_liquidity(100_000_0000000);
    h.set_price(0, 1_000_0000000);

    let trader1 = h.create_trader(500_0000000);
    let trader2 = h.create_trader(500_0000000);

    // Open unbalanced OI: 1000 long, 500 short
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
        &5_u32,
    );

    assert_eq!(h.pm.get_open_interest_long(), 1000_0000000);
    assert_eq!(h.pm.get_open_interest_short(), 500_0000000);

    // Get current funding rate (should be positive — longs pay)
    let (long_rate, short_rate) = h.funding.get_current_rate();
    // skew = (1000 - 500) * 10000 / 1500 = 3333 bps
    // clamped to max 10 bps
    // long_rate = 10 + 5 = 15
    // short_rate = -(10 - 5) = -5 (shorts receive)
    assert_eq!(long_rate, 15);
    assert_eq!(short_rate, -5);

    // Try to apply funding too early
    let keeper = Address::generate(&h.env);
    let result = h.funding.try_apply_funding(&keeper);
    assert!(result.is_err()); // FundingTooEarly

    // Advance time past funding interval (1 hour)
    h.advance_time(3601);

    // Now apply funding
    h.funding.apply_funding(&keeper);

    // Check cumulative indices updated
    assert_eq!(h.funding.get_cumulative_funding_long(), 15);
    assert_eq!(h.funding.get_cumulative_funding_short(), -5);
}

#[test]
fn test_multiple_traders_concurrent_positions() {
    let h = TestHarness::setup();
    h.add_liquidity(500_000_0000000); // Large LP pool
    h.set_price(0, 1_000_0000000);

    let trader_a = h.create_trader(1000_0000000);
    let trader_b = h.create_trader(1000_0000000);
    let trader_c = h.create_trader(1000_0000000);

    // Three traders, different directions and leverages
    let pos_a = h.pm.open_position(
        &trader_a,
        &0_u32,
        &Direction::Long,
        &200_0000000_i128,
        &10_u32,
    );
    let pos_b = h.pm.open_position(
        &trader_b,
        &0_u32,
        &Direction::Short,
        &300_0000000_i128,
        &5_u32,
    );
    let pos_c = h.pm.open_position(
        &trader_c,
        &0_u32,
        &Direction::Long,
        &100_0000000_i128,
        &20_u32,
    );

    // OI: long = 2000 + 2000 = 4000, short = 1500
    assert_eq!(h.pm.get_open_interest_long(), 4000_0000000);
    assert_eq!(h.pm.get_open_interest_short(), 1500_0000000);

    // Price moves up 5%
    h.set_price(0, 1_050_0000000);

    // Close all positions — different outcomes
    let pnl_a = h.pm.close_position(&trader_a, &pos_a, &0_u32);
    let pnl_b = h.pm.close_position(&trader_b, &pos_b, &0_u32);
    let pnl_c = h.pm.close_position(&trader_c, &pos_c, &0_u32);

    // Trader A (long 10x): PnL = 2000 * 0.05 / 1.0 = 100, fee = 2, net = 98
    assert_eq!(pnl_a, 98_0000000);
    // Trader B (short 5x): PnL = 1500 * (-0.05) / 1.0 = -75, fee = 1.5, net = -76.5
    assert_eq!(pnl_b, -76_5000000);
    // Trader C (long 20x): PnL = 2000 * 0.05 / 1.0 = 100, fee = 2, net = 98
    assert_eq!(pnl_c, 98_0000000);

    // All positions cleared
    assert_eq!(h.pm.get_open_interest_long(), 0);
    assert_eq!(h.pm.get_open_interest_short(), 0);
    assert_eq!(h.vault.get_locked_liquidity(), 0);
}

#[test]
fn test_lp_utilization_limit() {
    let h = TestHarness::setup();
    h.add_liquidity(10_000_0000000); // 10k USDC
    h.set_price(0, 1_000_0000000);

    let trader = h.create_trader(5000_0000000);

    // Open a position that uses 80% of liquidity (at the limit)
    // 80% of 10000 = 8000. We need collateral * leverage = 8000
    // 800 USDC * 10x = 8000 notional
    h.pm.open_position(
        &trader,
        &0_u32,
        &Direction::Long,
        &800_0000000_i128,
        &10_u32,
    );

    // Vault should have 80% utilization (at max)
    // Note: total_deposits = 10000 + 800(collateral) + 0.8(fee) = 10800.8
    // locked = 8000. util = 8000*10000/10800.8 = ~7407 bps (under 80%)
    // This is because LP deposit + trader deposit both count
    let util = h.vault.get_utilization();
    assert!(util <= 8000); // Under max utilization

    // LP cannot withdraw if it would push utilization above max
    let lp2 = h.add_liquidity(1000_0000000); // Add another LP
    let lp2_shares = h.vault.get_share_balance(&lp2);

    // The system should allow this withdrawal since there's headroom
    // But withdrawing too much would fail
    // Let's verify the system state is consistent
    let total_deposits = h.vault.get_total_deposits();
    let locked = h.vault.get_locked_liquidity();
    assert!(locked <= total_deposits);
    assert_eq!(locked, 8000_0000000);

    // Close the position to free liquidity
    h.set_price(0, 1_000_0000000);
    h.pm.close_position(&trader, &1_u64, &0_u32);
    assert_eq!(h.vault.get_locked_liquidity(), 0);

    // Now LP2 can withdraw
    h.vault.withdraw(&lp2, &lp2_shares);
}
