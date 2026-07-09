package models

import "time"

// Position represents an on-chain perpetual position.
type Position struct {
	ID         uint64 `json:"id"`
	Trader     string `json:"trader"`
	Asset      uint32 `json:"asset"`
	Direction  string `json:"direction"` // "long" or "short"
	Size       int64  `json:"size"`
	Collateral int64  `json:"collateral"`
	EntryPrice int64  `json:"entry_price"`
	Leverage   uint32 `json:"leverage"`
	OpenedAt   int64  `json:"opened_at"`
	Status     string `json:"status"` // "open", "closed", "liquidated"
	ClosedAt   *int64 `json:"closed_at,omitempty"`
	ExitPrice  *int64 `json:"exit_price,omitempty"`
	RealizedPnl *int64 `json:"realized_pnl,omitempty"`
	CloseFee    *int64 `json:"close_fee,omitempty"`
}

// Trade represents a completed trade (close or liquidation).
type Trade struct {
	ID          int64  `json:"id"`
	PositionID  uint64 `json:"position_id"`
	Trader      string `json:"trader"`
	Asset       uint32 `json:"asset"`
	Direction   string `json:"direction"`
	Size        int64  `json:"size"`
	EntryPrice  int64  `json:"entry_price"`
	ExitPrice   int64  `json:"exit_price"`
	RealizedPnl int64  `json:"realized_pnl"`
	Fee         int64  `json:"fee"`
	Type        string `json:"type"` // "close" or "liquidation"
	Timestamp   int64  `json:"timestamp"`
}

// FundingUpdate represents a funding rate application event.
type FundingUpdate struct {
	ID              int64 `json:"id"`
	LongRate        int64 `json:"long_rate"`
	ShortRate       int64 `json:"short_rate"`
	CumulativeLong  int64 `json:"cumulative_long"`
	CumulativeShort int64 `json:"cumulative_short"`
	OILong          int64 `json:"oi_long"`
	OIShort         int64 `json:"oi_short"`
	Timestamp       int64 `json:"timestamp"`
}

// VaultStats represents current vault state.
type VaultStats struct {
	TotalDeposits     int64 `json:"total_deposits"`
	TotalShares       int64 `json:"total_shares"`
	LockedLiquidity   int64 `json:"locked_liquidity"`
	AvailableLiquidity int64 `json:"available_liquidity"`
	SharePrice        int64 `json:"share_price"`
	UtilizationBps    int32 `json:"utilization_bps"`
}

// MarketStats represents current market stats for an asset.
type MarketStats struct {
	Asset           uint32 `json:"asset"`
	Price           int64  `json:"price"`
	OILong          int64  `json:"oi_long"`
	OIShort         int64  `json:"oi_short"`
	FundingRateLong int64  `json:"funding_rate_long"`
	FundingRateShort int64 `json:"funding_rate_short"`
	Volume24h       int64  `json:"volume_24h"`
	Trades24h       int64  `json:"trades_24h"`
	UpdatedAt       time.Time `json:"updated_at"`
}

// LiquidationEvent represents a liquidation that occurred.
type LiquidationEvent struct {
	ID         int64  `json:"id"`
	PositionID uint64 `json:"position_id"`
	Trader     string `json:"trader"`
	Keeper     string `json:"keeper"`
	Asset      uint32 `json:"asset"`
	MarkPrice  int64  `json:"mark_price"`
	MarginRatio int64 `json:"margin_ratio_bps"`
	Penalty    int64  `json:"penalty"`
	KeeperReward int64 `json:"keeper_reward"`
	Timestamp  int64  `json:"timestamp"`
}

// PriceUpdate is sent over WebSocket when price changes.
type PriceUpdate struct {
	Asset     uint32 `json:"asset"`
	Price     int64  `json:"price"`
	Timestamp int64  `json:"timestamp"`
}
