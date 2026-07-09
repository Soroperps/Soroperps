package store

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/soroperps/keeper/internal/models"
)

func testStore(t *testing.T) *Store {
	t.Helper()
	dir := t.TempDir()
	s, err := New(filepath.Join(dir, "test.db"))
	if err != nil {
		t.Fatalf("New() error: %v", err)
	}
	t.Cleanup(func() { s.Close() })
	return s
}

func TestNewAndMigrate(t *testing.T) {
	dir := t.TempDir()
	dbPath := filepath.Join(dir, "test.db")

	s, err := New(dbPath)
	if err != nil {
		t.Fatalf("New() error: %v", err)
	}
	s.Close()

	// Verify file created
	if _, err := os.Stat(dbPath); os.IsNotExist(err) {
		t.Error("database file not created")
	}

	// Open again — migrations should be idempotent
	s2, err := New(dbPath)
	if err != nil {
		t.Fatalf("second New() error: %v", err)
	}
	s2.Close()
}

func TestPositionCRUD(t *testing.T) {
	s := testStore(t)

	// Insert
	pos := &models.Position{
		ID: 1, Trader: "GABC", Asset: 0, Direction: "long",
		Size: 1000, Collateral: 100, EntryPrice: 1500000,
		Leverage: 10, OpenedAt: 1000, Status: "open",
	}
	if err := s.UpsertPosition(pos); err != nil {
		t.Fatalf("UpsertPosition: %v", err)
	}

	// Read
	got, err := s.GetPosition(1)
	if err != nil {
		t.Fatalf("GetPosition: %v", err)
	}
	if got == nil {
		t.Fatal("GetPosition returned nil")
	}
	if got.Trader != "GABC" || got.Size != 1000 || got.Status != "open" {
		t.Errorf("unexpected position: %+v", got)
	}

	// Update (close position)
	exitPrice := int64(1600000)
	pnl := int64(50)
	fee := int64(1)
	closedAt := int64(2000)
	pos.Status = "closed"
	pos.ExitPrice = &exitPrice
	pos.RealizedPnl = &pnl
	pos.CloseFee = &fee
	pos.ClosedAt = &closedAt
	if err := s.UpsertPosition(pos); err != nil {
		t.Fatalf("UpsertPosition (update): %v", err)
	}

	got, _ = s.GetPosition(1)
	if got.Status != "closed" || *got.ExitPrice != 1600000 || *got.RealizedPnl != 50 {
		t.Errorf("position not updated correctly: %+v", got)
	}

	// Not found
	got, err = s.GetPosition(999)
	if err != nil {
		t.Fatalf("GetPosition(999): %v", err)
	}
	if got != nil {
		t.Errorf("expected nil for nonexistent position, got %+v", got)
	}
}

func TestGetOpenPositions(t *testing.T) {
	s := testStore(t)

	s.UpsertPosition(&models.Position{ID: 1, Trader: "GA", Asset: 0, Direction: "long", Size: 100, Collateral: 10, EntryPrice: 100, Leverage: 10, OpenedAt: 1000, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 2, Trader: "GA", Asset: 0, Direction: "short", Size: 200, Collateral: 20, EntryPrice: 100, Leverage: 10, OpenedAt: 1001, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 3, Trader: "GB", Asset: 1, Direction: "long", Size: 300, Collateral: 30, EntryPrice: 50000, Leverage: 5, OpenedAt: 1002, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 4, Trader: "GA", Asset: 0, Direction: "long", Size: 100, Collateral: 10, EntryPrice: 100, Leverage: 10, OpenedAt: 999, Status: "closed"})

	// All open
	all, err := s.GetOpenPositions("")
	if err != nil {
		t.Fatalf("GetOpenPositions: %v", err)
	}
	if len(all) != 3 {
		t.Errorf("expected 3 open positions, got %d", len(all))
	}

	// Filter by trader
	ga, err := s.GetOpenPositions("GA")
	if err != nil {
		t.Fatalf("GetOpenPositions(GA): %v", err)
	}
	if len(ga) != 2 {
		t.Errorf("expected 2 open positions for GA, got %d", len(ga))
	}
}

func TestTrades(t *testing.T) {
	s := testStore(t)

	s.InsertTrade(&models.Trade{PositionID: 1, Trader: "GA", Asset: 0, Direction: "long", Size: 1000, EntryPrice: 100, ExitPrice: 110, RealizedPnl: 99, Fee: 1, Type: "close", Timestamp: 2000})
	s.InsertTrade(&models.Trade{PositionID: 2, Trader: "GB", Asset: 0, Direction: "short", Size: 500, EntryPrice: 100, ExitPrice: 95, RealizedPnl: 49, Fee: 1, Type: "close", Timestamp: 2001})

	trades, err := s.GetRecentTrades(10)
	if err != nil {
		t.Fatalf("GetRecentTrades: %v", err)
	}
	if len(trades) != 2 {
		t.Errorf("expected 2 trades, got %d", len(trades))
	}
	// Most recent first
	if trades[0].Timestamp != 2001 {
		t.Errorf("expected most recent trade first, got timestamp %d", trades[0].Timestamp)
	}

	// By trader
	gaTrades, err := s.GetTradesByTrader("GA", 10)
	if err != nil {
		t.Fatalf("GetTradesByTrader: %v", err)
	}
	if len(gaTrades) != 1 {
		t.Errorf("expected 1 trade for GA, got %d", len(gaTrades))
	}
}

func TestFundingUpdates(t *testing.T) {
	s := testStore(t)

	s.InsertFundingUpdate(&models.FundingUpdate{LongRate: 15, ShortRate: -5, CumulativeLong: 15, CumulativeShort: -5, OILong: 1000, OIShort: 500, Timestamp: 5000})
	s.InsertFundingUpdate(&models.FundingUpdate{LongRate: 10, ShortRate: -2, CumulativeLong: 25, CumulativeShort: -7, OILong: 1200, OIShort: 800, Timestamp: 8600})

	history, err := s.GetFundingHistory(10)
	if err != nil {
		t.Fatalf("GetFundingHistory: %v", err)
	}
	if len(history) != 2 {
		t.Errorf("expected 2 funding updates, got %d", len(history))
	}
	if history[0].Timestamp != 8600 {
		t.Errorf("expected most recent first")
	}
}

func TestLiquidations(t *testing.T) {
	s := testStore(t)

	s.InsertLiquidation(&models.LiquidationEvent{PositionID: 1, Trader: "GA", Keeper: "GK", Asset: 0, MarkPrice: 920, MarginRatio: 200, Penalty: 25, KeeperReward: 5, Timestamp: 3000})

	events, err := s.GetRecentLiquidations(10)
	if err != nil {
		t.Fatalf("GetRecentLiquidations: %v", err)
	}
	if len(events) != 1 {
		t.Errorf("expected 1 liquidation, got %d", len(events))
	}
	if events[0].Keeper != "GK" {
		t.Errorf("expected keeper GK, got %s", events[0].Keeper)
	}
}

func TestCursor(t *testing.T) {
	s := testStore(t)

	ledger, cursor, err := s.GetCursor()
	if err != nil {
		t.Fatalf("GetCursor: %v", err)
	}
	if ledger != 0 || cursor != "" {
		t.Errorf("expected initial cursor (0, ''), got (%d, %q)", ledger, cursor)
	}

	if err := s.SetCursor(12345, "abc123"); err != nil {
		t.Fatalf("SetCursor: %v", err)
	}

	ledger, cursor, err = s.GetCursor()
	if err != nil {
		t.Fatalf("GetCursor after set: %v", err)
	}
	if ledger != 12345 || cursor != "abc123" {
		t.Errorf("expected (12345, abc123), got (%d, %q)", ledger, cursor)
	}
}

func TestOpenInterest(t *testing.T) {
	s := testStore(t)

	s.UpsertPosition(&models.Position{ID: 1, Trader: "GA", Asset: 0, Direction: "long", Size: 1000, Collateral: 100, EntryPrice: 100, Leverage: 10, OpenedAt: 1000, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 2, Trader: "GB", Asset: 0, Direction: "long", Size: 500, Collateral: 50, EntryPrice: 100, Leverage: 10, OpenedAt: 1001, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 3, Trader: "GC", Asset: 0, Direction: "short", Size: 800, Collateral: 80, EntryPrice: 100, Leverage: 10, OpenedAt: 1002, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 4, Trader: "GD", Asset: 0, Direction: "long", Size: 200, Collateral: 20, EntryPrice: 100, Leverage: 10, OpenedAt: 1003, Status: "closed"})

	long, short, err := s.GetOpenInterest(0)
	if err != nil {
		t.Fatalf("GetOpenInterest: %v", err)
	}
	if long != 1500 {
		t.Errorf("expected long OI 1500, got %d", long)
	}
	if short != 800 {
		t.Errorf("expected short OI 800, got %d", short)
	}
}
