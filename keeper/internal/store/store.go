package store

import (
	"database/sql"
	"fmt"
	"time"

	_ "github.com/mattn/go-sqlite3"

	"github.com/soroperps/keeper/internal/models"
)

// Store provides persistence for indexed on-chain data.
type Store struct {
	db *sql.DB
}

// New opens (or creates) a SQLite database and runs migrations.
func New(dbPath string) (*Store, error) {
	db, err := sql.Open("sqlite3", dbPath+"?_journal_mode=WAL&_busy_timeout=5000")
	if err != nil {
		return nil, fmt.Errorf("open db: %w", err)
	}

	s := &Store{db: db}
	if err := s.migrate(); err != nil {
		db.Close()
		return nil, fmt.Errorf("migrate: %w", err)
	}
	return s, nil
}

// Close closes the database connection.
func (s *Store) Close() error {
	return s.db.Close()
}

func (s *Store) migrate() error {
	migrations := []string{
		`CREATE TABLE IF NOT EXISTS positions (
			id            INTEGER PRIMARY KEY,
			trader        TEXT NOT NULL,
			asset         INTEGER NOT NULL,
			direction     TEXT NOT NULL,
			size          INTEGER NOT NULL,
			collateral    INTEGER NOT NULL,
			entry_price   INTEGER NOT NULL,
			leverage      INTEGER NOT NULL,
			opened_at     INTEGER NOT NULL,
			status        TEXT NOT NULL DEFAULT 'open',
			closed_at     INTEGER,
			exit_price    INTEGER,
			realized_pnl  INTEGER,
			close_fee     INTEGER
		)`,
		`CREATE INDEX IF NOT EXISTS idx_positions_trader ON positions(trader)`,
		`CREATE INDEX IF NOT EXISTS idx_positions_status ON positions(status)`,
		`CREATE INDEX IF NOT EXISTS idx_positions_asset ON positions(asset)`,

		`CREATE TABLE IF NOT EXISTS trades (
			id            INTEGER PRIMARY KEY AUTOINCREMENT,
			position_id   INTEGER NOT NULL,
			trader        TEXT NOT NULL,
			asset         INTEGER NOT NULL,
			direction     TEXT NOT NULL,
			size          INTEGER NOT NULL,
			entry_price   INTEGER NOT NULL,
			exit_price    INTEGER NOT NULL,
			realized_pnl  INTEGER NOT NULL,
			fee           INTEGER NOT NULL,
			type          TEXT NOT NULL,
			timestamp     INTEGER NOT NULL
		)`,
		`CREATE INDEX IF NOT EXISTS idx_trades_trader ON trades(trader)`,
		`CREATE INDEX IF NOT EXISTS idx_trades_timestamp ON trades(timestamp)`,

		`CREATE TABLE IF NOT EXISTS funding_updates (
			id               INTEGER PRIMARY KEY AUTOINCREMENT,
			long_rate        INTEGER NOT NULL,
			short_rate       INTEGER NOT NULL,
			cumulative_long  INTEGER NOT NULL,
			cumulative_short INTEGER NOT NULL,
			oi_long          INTEGER NOT NULL,
			oi_short         INTEGER NOT NULL,
			timestamp        INTEGER NOT NULL
		)`,

		`CREATE TABLE IF NOT EXISTS liquidations (
			id            INTEGER PRIMARY KEY AUTOINCREMENT,
			position_id   INTEGER NOT NULL,
			trader        TEXT NOT NULL,
			keeper        TEXT NOT NULL,
			asset         INTEGER NOT NULL,
			mark_price    INTEGER NOT NULL,
			margin_ratio  INTEGER NOT NULL,
			penalty       INTEGER NOT NULL,
			keeper_reward INTEGER NOT NULL,
			timestamp     INTEGER NOT NULL
		)`,
		`CREATE INDEX IF NOT EXISTS idx_liquidations_trader ON liquidations(trader)`,

		`CREATE TABLE IF NOT EXISTS cursor (
			id          INTEGER PRIMARY KEY CHECK (id = 1),
			last_ledger INTEGER NOT NULL DEFAULT 0,
			last_cursor TEXT NOT NULL DEFAULT ''
		)`,
		`INSERT OR IGNORE INTO cursor (id, last_ledger, last_cursor) VALUES (1, 0, '')`,
	}

	for _, m := range migrations {
		if _, err := s.db.Exec(m); err != nil {
			return fmt.Errorf("exec migration: %w", err)
		}
	}
	return nil
}

// ---------------------------------------------------------------------------
// Positions
// ---------------------------------------------------------------------------

// UpsertPosition inserts or updates a position.
func (s *Store) UpsertPosition(p *models.Position) error {
	_, err := s.db.Exec(`
		INSERT INTO positions (id, trader, asset, direction, size, collateral, entry_price, leverage, opened_at, status, closed_at, exit_price, realized_pnl, close_fee)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET
			status=excluded.status,
			closed_at=excluded.closed_at,
			exit_price=excluded.exit_price,
			realized_pnl=excluded.realized_pnl,
			close_fee=excluded.close_fee`,
		p.ID, p.Trader, p.Asset, p.Direction, p.Size, p.Collateral,
		p.EntryPrice, p.Leverage, p.OpenedAt, p.Status,
		p.ClosedAt, p.ExitPrice, p.RealizedPnl, p.CloseFee,
	)
	return err
}

// GetOpenPositions returns all open positions, optionally filtered by trader.
func (s *Store) GetOpenPositions(trader string) ([]models.Position, error) {
	query := `SELECT id, trader, asset, direction, size, collateral, entry_price, leverage, opened_at, status FROM positions WHERE status = 'open'`
	args := []interface{}{}
	if trader != "" {
		query += ` AND trader = ?`
		args = append(args, trader)
	}
	query += ` ORDER BY opened_at DESC`

	rows, err := s.db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var positions []models.Position
	for rows.Next() {
		var p models.Position
		if err := rows.Scan(&p.ID, &p.Trader, &p.Asset, &p.Direction, &p.Size, &p.Collateral, &p.EntryPrice, &p.Leverage, &p.OpenedAt, &p.Status); err != nil {
			return nil, err
		}
		positions = append(positions, p)
	}
	return positions, rows.Err()
}

// GetPosition returns a single position by ID.
func (s *Store) GetPosition(id uint64) (*models.Position, error) {
	var p models.Position
	err := s.db.QueryRow(`
		SELECT id, trader, asset, direction, size, collateral, entry_price, leverage, opened_at, status, closed_at, exit_price, realized_pnl, close_fee
		FROM positions WHERE id = ?`, id,
	).Scan(&p.ID, &p.Trader, &p.Asset, &p.Direction, &p.Size, &p.Collateral, &p.EntryPrice, &p.Leverage, &p.OpenedAt, &p.Status, &p.ClosedAt, &p.ExitPrice, &p.RealizedPnl, &p.CloseFee)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &p, nil
}

// GetPositionsByTrader returns all positions for a trader.
func (s *Store) GetPositionsByTrader(trader string, limit int) ([]models.Position, error) {
	if limit <= 0 {
		limit = 50
	}
	rows, err := s.db.Query(`
		SELECT id, trader, asset, direction, size, collateral, entry_price, leverage, opened_at, status, closed_at, exit_price, realized_pnl, close_fee
		FROM positions WHERE trader = ? ORDER BY opened_at DESC LIMIT ?`, trader, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var positions []models.Position
	for rows.Next() {
		var p models.Position
		if err := rows.Scan(&p.ID, &p.Trader, &p.Asset, &p.Direction, &p.Size, &p.Collateral, &p.EntryPrice, &p.Leverage, &p.OpenedAt, &p.Status, &p.ClosedAt, &p.ExitPrice, &p.RealizedPnl, &p.CloseFee); err != nil {
			return nil, err
		}
		positions = append(positions, p)
	}
	return positions, rows.Err()
}

// ---------------------------------------------------------------------------
// Trades
// ---------------------------------------------------------------------------

// InsertTrade records a completed trade.
func (s *Store) InsertTrade(t *models.Trade) error {
	_, err := s.db.Exec(`
		INSERT INTO trades (position_id, trader, asset, direction, size, entry_price, exit_price, realized_pnl, fee, type, timestamp)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		t.PositionID, t.Trader, t.Asset, t.Direction, t.Size,
		t.EntryPrice, t.ExitPrice, t.RealizedPnl, t.Fee, t.Type, t.Timestamp,
	)
	return err
}

// GetRecentTrades returns the most recent trades.
func (s *Store) GetRecentTrades(limit int) ([]models.Trade, error) {
	if limit <= 0 {
		limit = 50
	}
	rows, err := s.db.Query(`
		SELECT id, position_id, trader, asset, direction, size, entry_price, exit_price, realized_pnl, fee, type, timestamp
		FROM trades ORDER BY timestamp DESC LIMIT ?`, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var trades []models.Trade
	for rows.Next() {
		var t models.Trade
		if err := rows.Scan(&t.ID, &t.PositionID, &t.Trader, &t.Asset, &t.Direction, &t.Size, &t.EntryPrice, &t.ExitPrice, &t.RealizedPnl, &t.Fee, &t.Type, &t.Timestamp); err != nil {
			return nil, err
		}
		trades = append(trades, t)
	}
	return trades, rows.Err()
}

// GetTradesByTrader returns trades for a specific trader.
func (s *Store) GetTradesByTrader(trader string, limit int) ([]models.Trade, error) {
	if limit <= 0 {
		limit = 50
	}
	rows, err := s.db.Query(`
		SELECT id, position_id, trader, asset, direction, size, entry_price, exit_price, realized_pnl, fee, type, timestamp
		FROM trades WHERE trader = ? ORDER BY timestamp DESC LIMIT ?`, trader, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var trades []models.Trade
	for rows.Next() {
		var t models.Trade
		if err := rows.Scan(&t.ID, &t.PositionID, &t.Trader, &t.Asset, &t.Direction, &t.Size, &t.EntryPrice, &t.ExitPrice, &t.RealizedPnl, &t.Fee, &t.Type, &t.Timestamp); err != nil {
			return nil, err
		}
		trades = append(trades, t)
	}
	return trades, rows.Err()
}

// GetVolume24h returns the total trading volume in the last 24 hours.
func (s *Store) GetVolume24h() (int64, int64, error) {
	cutoff := time.Now().Unix() - 86400
	var volume, count int64
	err := s.db.QueryRow(`
		SELECT COALESCE(SUM(size), 0), COUNT(*) FROM trades WHERE timestamp > ?`, cutoff,
	).Scan(&volume, &count)
	return volume, count, err
}

// ---------------------------------------------------------------------------
// Funding updates
// ---------------------------------------------------------------------------

// InsertFundingUpdate records a funding rate application.
func (s *Store) InsertFundingUpdate(f *models.FundingUpdate) error {
	_, err := s.db.Exec(`
		INSERT INTO funding_updates (long_rate, short_rate, cumulative_long, cumulative_short, oi_long, oi_short, timestamp)
		VALUES (?, ?, ?, ?, ?, ?, ?)`,
		f.LongRate, f.ShortRate, f.CumulativeLong, f.CumulativeShort, f.OILong, f.OIShort, f.Timestamp,
	)
	return err
}

// GetFundingHistory returns recent funding updates.
func (s *Store) GetFundingHistory(limit int) ([]models.FundingUpdate, error) {
	if limit <= 0 {
		limit = 50
	}
	rows, err := s.db.Query(`
		SELECT id, long_rate, short_rate, cumulative_long, cumulative_short, oi_long, oi_short, timestamp
		FROM funding_updates ORDER BY timestamp DESC LIMIT ?`, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var updates []models.FundingUpdate
	for rows.Next() {
		var f models.FundingUpdate
		if err := rows.Scan(&f.ID, &f.LongRate, &f.ShortRate, &f.CumulativeLong, &f.CumulativeShort, &f.OILong, &f.OIShort, &f.Timestamp); err != nil {
			return nil, err
		}
		updates = append(updates, f)
	}
	return updates, rows.Err()
}

// ---------------------------------------------------------------------------
// Liquidations
// ---------------------------------------------------------------------------

// InsertLiquidation records a liquidation event.
func (s *Store) InsertLiquidation(l *models.LiquidationEvent) error {
	_, err := s.db.Exec(`
		INSERT INTO liquidations (position_id, trader, keeper, asset, mark_price, margin_ratio, penalty, keeper_reward, timestamp)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		l.PositionID, l.Trader, l.Keeper, l.Asset, l.MarkPrice, l.MarginRatio, l.Penalty, l.KeeperReward, l.Timestamp,
	)
	return err
}

// GetRecentLiquidations returns recent liquidation events.
func (s *Store) GetRecentLiquidations(limit int) ([]models.LiquidationEvent, error) {
	if limit <= 0 {
		limit = 50
	}
	rows, err := s.db.Query(`
		SELECT id, position_id, trader, keeper, asset, mark_price, margin_ratio, penalty, keeper_reward, timestamp
		FROM liquidations ORDER BY timestamp DESC LIMIT ?`, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var events []models.LiquidationEvent
	for rows.Next() {
		var l models.LiquidationEvent
		if err := rows.Scan(&l.ID, &l.PositionID, &l.Trader, &l.Keeper, &l.Asset, &l.MarkPrice, &l.MarginRatio, &l.Penalty, &l.KeeperReward, &l.Timestamp); err != nil {
			return nil, err
		}
		events = append(events, l)
	}
	return events, rows.Err()
}

// ---------------------------------------------------------------------------
// Cursor (indexer bookmark)
// ---------------------------------------------------------------------------

// GetCursor returns the last indexed ledger and cursor.
func (s *Store) GetCursor() (int64, string, error) {
	var ledger int64
	var cursor string
	err := s.db.QueryRow(`SELECT last_ledger, last_cursor FROM cursor WHERE id = 1`).Scan(&ledger, &cursor)
	return ledger, cursor, err
}

// SetCursor updates the indexer bookmark.
func (s *Store) SetCursor(ledger int64, cursor string) error {
	_, err := s.db.Exec(`UPDATE cursor SET last_ledger = ?, last_cursor = ? WHERE id = 1`, ledger, cursor)
	return err
}

// ---------------------------------------------------------------------------
// Aggregate queries
// ---------------------------------------------------------------------------

// GetOpenInterest returns total long and short OI from open positions.
func (s *Store) GetOpenInterest(asset uint32) (long int64, short int64, err error) {
	err = s.db.QueryRow(`
		SELECT
			COALESCE(SUM(CASE WHEN direction = 'long' THEN size ELSE 0 END), 0),
			COALESCE(SUM(CASE WHEN direction = 'short' THEN size ELSE 0 END), 0)
		FROM positions WHERE status = 'open' AND asset = ?`, asset,
	).Scan(&long, &short)
	return
}

// GetOpenPositionCount returns the number of open positions.
func (s *Store) GetOpenPositionCount() (int64, error) {
	var count int64
	err := s.db.QueryRow(`SELECT COUNT(*) FROM positions WHERE status = 'open'`).Scan(&count)
	return count, err
}
