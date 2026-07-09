use perps_types::DECIMALS;

/// Calculate shares to mint for a given deposit amount.
///
/// If no shares exist yet (first deposit), mint 1:1.
/// Otherwise: `shares = amount * total_shares / total_deposits`
pub fn calc_shares_to_mint(amount: i128, total_shares: i128, total_deposits: i128) -> i128 {
    if total_shares == 0 || total_deposits == 0 {
        amount
    } else {
        amount * total_shares / total_deposits
    }
}

/// Calculate USDC to return for a given number of shares being burned.
///
/// `usdc = shares * total_deposits / total_shares`
pub fn calc_withdrawal_amount(shares: i128, total_shares: i128, total_deposits: i128) -> i128 {
    if total_shares == 0 {
        0
    } else {
        shares * total_deposits / total_shares
    }
}

/// Calculate the current share price (7 decimals).
///
/// `price = total_deposits * DECIMALS / total_shares`
pub fn calc_share_price(total_shares: i128, total_deposits: i128) -> i128 {
    if total_shares == 0 {
        DECIMALS
    } else {
        total_deposits * DECIMALS / total_shares
    }
}

/// Calculate utilization in basis points.
///
/// `utilization = locked * 10000 / total_deposits`
pub fn calc_utilization_bps(locked: i128, total_deposits: i128) -> u32 {
    if total_deposits == 0 {
        0
    } else {
        (locked * 10_000 / total_deposits) as u32
    }
}
