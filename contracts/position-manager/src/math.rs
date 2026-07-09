use perps_types::Direction;

/// Calculate unrealized PnL for a position.
///
/// For Long:  pnl = size * (current_price - entry_price) / entry_price
/// For Short: pnl = size * (entry_price - current_price) / entry_price
pub fn calculate_pnl(
    direction: &Direction,
    size: i128,
    entry_price: i128,
    current_price: i128,
) -> i128 {
    match direction {
        Direction::Long => size * (current_price - entry_price) / entry_price,
        Direction::Short => size * (entry_price - current_price) / entry_price,
    }
}

/// Calculate trading fee: `notional_size * fee_bps / 10_000`
pub fn calculate_fee(notional_size: i128, fee_bps: u32) -> i128 {
    notional_size * (fee_bps as i128) / 10_000
}

/// Calculate funding payment owed by a position.
///
/// `payment = size * (current_cumulative - entry_cumulative) / 10_000`
/// Positive = position owes funding; Negative = position receives funding.
pub fn calculate_funding_payment(
    size: i128,
    entry_funding_index: i128,
    current_funding_index: i128,
) -> i128 {
    size * (current_funding_index - entry_funding_index) / 10_000
}
