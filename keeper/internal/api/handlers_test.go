package api

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"github.com/soroperps/keeper/internal/models"
	"github.com/soroperps/keeper/internal/store"
)

func testSetup(t *testing.T) (*Handlers, *store.Store) {
	t.Helper()
	dir := t.TempDir()
	s, err := store.New(filepath.Join(dir, "test.db"))
	if err != nil {
		t.Fatalf("store.New: %v", err)
	}
	t.Cleanup(func() { s.Close() })
	h := NewHandlers(s)
	return h, s
}

func doRequest(t *testing.T, mux *http.ServeMux, method, path string) *httptest.ResponseRecorder {
	t.Helper()
	req := httptest.NewRequest(method, path, nil)
	rr := httptest.NewRecorder()
	mux.ServeHTTP(rr, req)
	return rr
}

func TestHealthEndpoint(t *testing.T) {
	h, _ := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	rr := doRequest(t, mux, "GET", "/api/v1/health")

	if rr.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", rr.Code)
	}

	var body map[string]string
	json.Unmarshal(rr.Body.Bytes(), &body)
	if body["status"] != "ok" {
		t.Errorf("expected status ok, got %q", body["status"])
	}
}

func TestGetPositionsEmpty(t *testing.T) {
	h, _ := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	rr := doRequest(t, mux, "GET", "/api/v1/positions")

	if rr.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", rr.Code)
	}

	var positions []models.Position
	json.Unmarshal(rr.Body.Bytes(), &positions)
	if len(positions) != 0 {
		t.Errorf("expected 0 positions, got %d", len(positions))
	}
}

func TestGetPositionsWithData(t *testing.T) {
	h, s := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	s.UpsertPosition(&models.Position{ID: 1, Trader: "GABC", Asset: 0, Direction: "long", Size: 1000, Collateral: 100, EntryPrice: 150, Leverage: 10, OpenedAt: 1000, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 2, Trader: "GDEF", Asset: 0, Direction: "short", Size: 500, Collateral: 50, EntryPrice: 150, Leverage: 10, OpenedAt: 1001, Status: "open"})

	// All open
	rr := doRequest(t, mux, "GET", "/api/v1/positions")
	var positions []models.Position
	json.Unmarshal(rr.Body.Bytes(), &positions)
	if len(positions) != 2 {
		t.Errorf("expected 2 positions, got %d", len(positions))
	}

	// Filter by trader
	rr = doRequest(t, mux, "GET", "/api/v1/positions?trader=GABC")
	json.Unmarshal(rr.Body.Bytes(), &positions)
	if len(positions) != 1 {
		t.Errorf("expected 1 position for GABC, got %d", len(positions))
	}
}

func TestGetPositionByID(t *testing.T) {
	h, s := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	s.UpsertPosition(&models.Position{ID: 42, Trader: "GABC", Asset: 0, Direction: "long", Size: 1000, Collateral: 100, EntryPrice: 150, Leverage: 10, OpenedAt: 1000, Status: "open"})

	// Found
	rr := doRequest(t, mux, "GET", "/api/v1/positions/42")
	if rr.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", rr.Code)
	}
	var pos models.Position
	json.Unmarshal(rr.Body.Bytes(), &pos)
	if pos.ID != 42 {
		t.Errorf("expected position 42, got %d", pos.ID)
	}

	// Not found
	rr = doRequest(t, mux, "GET", "/api/v1/positions/999")
	if rr.Code != http.StatusNotFound {
		t.Errorf("expected 404, got %d", rr.Code)
	}

	// Invalid ID
	rr = doRequest(t, mux, "GET", "/api/v1/positions/abc")
	if rr.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", rr.Code)
	}
}

func TestGetTrades(t *testing.T) {
	h, s := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	s.InsertTrade(&models.Trade{PositionID: 1, Trader: "GABC", Asset: 0, Direction: "long", Size: 1000, EntryPrice: 100, ExitPrice: 110, RealizedPnl: 99, Fee: 1, Type: "close", Timestamp: 2000})
	s.InsertTrade(&models.Trade{PositionID: 2, Trader: "GDEF", Asset: 0, Direction: "short", Size: 500, EntryPrice: 100, ExitPrice: 90, RealizedPnl: 49, Fee: 1, Type: "close", Timestamp: 2001})

	// All trades
	rr := doRequest(t, mux, "GET", "/api/v1/trades")
	var trades []models.Trade
	json.Unmarshal(rr.Body.Bytes(), &trades)
	if len(trades) != 2 {
		t.Errorf("expected 2 trades, got %d", len(trades))
	}

	// By trader
	rr = doRequest(t, mux, "GET", "/api/v1/trades?trader=GABC")
	json.Unmarshal(rr.Body.Bytes(), &trades)
	if len(trades) != 1 {
		t.Errorf("expected 1 trade for GABC, got %d", len(trades))
	}

	// With limit
	rr = doRequest(t, mux, "GET", "/api/v1/trades?limit=1")
	json.Unmarshal(rr.Body.Bytes(), &trades)
	if len(trades) != 1 {
		t.Errorf("expected 1 trade with limit=1, got %d", len(trades))
	}
}

func TestGetFundingHistory(t *testing.T) {
	h, s := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	s.InsertFundingUpdate(&models.FundingUpdate{LongRate: 15, ShortRate: -5, CumulativeLong: 15, CumulativeShort: -5, OILong: 1000, OIShort: 500, Timestamp: 5000})

	rr := doRequest(t, mux, "GET", "/api/v1/funding")
	var history []models.FundingUpdate
	json.Unmarshal(rr.Body.Bytes(), &history)
	if len(history) != 1 {
		t.Errorf("expected 1 funding update, got %d", len(history))
	}
}

func TestGetLiquidations(t *testing.T) {
	h, s := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	s.InsertLiquidation(&models.LiquidationEvent{PositionID: 1, Trader: "GA", Keeper: "GK", Asset: 0, MarkPrice: 920, MarginRatio: 200, Penalty: 25, KeeperReward: 5, Timestamp: 3000})

	rr := doRequest(t, mux, "GET", "/api/v1/liquidations")
	var events []models.LiquidationEvent
	json.Unmarshal(rr.Body.Bytes(), &events)
	if len(events) != 1 {
		t.Errorf("expected 1 liquidation, got %d", len(events))
	}
}

func TestGetStats(t *testing.T) {
	h, s := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	s.UpsertPosition(&models.Position{ID: 1, Trader: "GA", Asset: 0, Direction: "long", Size: 1000, Collateral: 100, EntryPrice: 100, Leverage: 10, OpenedAt: 1000, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 2, Trader: "GB", Asset: 0, Direction: "short", Size: 500, Collateral: 50, EntryPrice: 100, Leverage: 10, OpenedAt: 1001, Status: "open"})

	rr := doRequest(t, mux, "GET", "/api/v1/stats")
	var stats map[string]interface{}
	json.Unmarshal(rr.Body.Bytes(), &stats)

	if stats["open_positions"].(float64) != 2 {
		t.Errorf("expected 2 open positions, got %v", stats["open_positions"])
	}
}

func TestGetMarketStats(t *testing.T) {
	h, s := testSetup(t)
	mux := http.NewServeMux()
	h.RegisterRoutes(mux)

	s.UpsertPosition(&models.Position{ID: 1, Trader: "GA", Asset: 0, Direction: "long", Size: 1000, Collateral: 100, EntryPrice: 100, Leverage: 10, OpenedAt: 1000, Status: "open"})
	s.UpsertPosition(&models.Position{ID: 2, Trader: "GB", Asset: 0, Direction: "short", Size: 800, Collateral: 80, EntryPrice: 100, Leverage: 10, OpenedAt: 1001, Status: "open"})

	rr := doRequest(t, mux, "GET", "/api/v1/market/0")
	var stats map[string]interface{}
	json.Unmarshal(rr.Body.Bytes(), &stats)

	if stats["oi_long"].(float64) != 1000 {
		t.Errorf("expected oi_long 1000, got %v", stats["oi_long"])
	}
	if stats["oi_short"].(float64) != 800 {
		t.Errorf("expected oi_short 800, got %v", stats["oi_short"])
	}

	// Invalid asset
	rr = doRequest(t, mux, "GET", "/api/v1/market/abc")
	if rr.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", rr.Code)
	}
}
