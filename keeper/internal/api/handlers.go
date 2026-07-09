package api

import (
	"encoding/json"
	"log"
	"net/http"
	"strconv"

	"github.com/soroperps/keeper/internal/models"
	"github.com/soroperps/keeper/internal/store"
)

// Handlers holds HTTP handler dependencies.
type Handlers struct {
	store *store.Store
}

// NewHandlers creates API handlers.
func NewHandlers(s *store.Store) *Handlers {
	return &Handlers{store: s}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

func writeJSON(w http.ResponseWriter, status int, v interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(v); err != nil {
		log.Printf("[api] encode error: %v", err)
	}
}

func writeError(w http.ResponseWriter, status int, msg string) {
	writeJSON(w, status, map[string]string{"error": msg})
}

func queryInt(r *http.Request, key string, defaultVal int) int {
	v := r.URL.Query().Get(key)
	if v == "" {
		return defaultVal
	}
	n, err := strconv.Atoi(v)
	if err != nil || n <= 0 {
		return defaultVal
	}
	return n
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

// RegisterRoutes registers all API routes on the given mux.
func (h *Handlers) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/v1/health", h.Health)
	mux.HandleFunc("GET /api/v1/positions", h.GetPositions)
	mux.HandleFunc("GET /api/v1/positions/{id}", h.GetPosition)
	mux.HandleFunc("GET /api/v1/trades", h.GetTrades)
	mux.HandleFunc("GET /api/v1/funding", h.GetFundingHistory)
	mux.HandleFunc("GET /api/v1/liquidations", h.GetLiquidations)
	mux.HandleFunc("GET /api/v1/stats", h.GetStats)
	mux.HandleFunc("GET /api/v1/market/{asset}", h.GetMarketStats)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

// Health returns a simple health check.
func (h *Handlers) Health(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// GetPositions returns positions, optionally filtered by trader and status.
//
//	GET /api/v1/positions?trader=GABC&status=open&limit=50
func (h *Handlers) GetPositions(w http.ResponseWriter, r *http.Request) {
	trader := r.URL.Query().Get("trader")
	status := r.URL.Query().Get("status")
	limit := queryInt(r, "limit", 50)

	if status == "" || status == "open" {
		positions, err := h.store.GetOpenPositions(trader)
		if err != nil {
			writeError(w, http.StatusInternalServerError, "failed to fetch positions")
			log.Printf("[api] GetOpenPositions error: %v", err)
			return
		}
		if positions == nil {
			positions = []models.Position{}
		}
		writeJSON(w, http.StatusOK, positions)
		return
	}

	if trader != "" {
		positions, err := h.store.GetPositionsByTrader(trader, limit)
		if err != nil {
			writeError(w, http.StatusInternalServerError, "failed to fetch positions")
			log.Printf("[api] GetPositionsByTrader error: %v", err)
			return
		}
		if positions == nil {
			positions = []models.Position{}
		}
		writeJSON(w, http.StatusOK, positions)
		return
	}

	writeError(w, http.StatusBadRequest, "provide trader query param for non-open status queries")
}

// GetPosition returns a single position by ID.
//
//	GET /api/v1/positions/123
func (h *Handlers) GetPosition(w http.ResponseWriter, r *http.Request) {
	idStr := r.PathValue("id")
	id, err := strconv.ParseUint(idStr, 10, 64)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid position id")
		return
	}

	pos, err := h.store.GetPosition(id)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to fetch position")
		log.Printf("[api] GetPosition error: %v", err)
		return
	}
	if pos == nil {
		writeError(w, http.StatusNotFound, "position not found")
		return
	}
	writeJSON(w, http.StatusOK, pos)
}

// GetTrades returns recent trades.
//
//	GET /api/v1/trades?trader=GABC&limit=20
func (h *Handlers) GetTrades(w http.ResponseWriter, r *http.Request) {
	trader := r.URL.Query().Get("trader")
	limit := queryInt(r, "limit", 50)

	if trader != "" {
		trades, err := h.store.GetTradesByTrader(trader, limit)
		if err != nil {
			writeError(w, http.StatusInternalServerError, "failed to fetch trades")
			log.Printf("[api] GetTradesByTrader error: %v", err)
			return
		}
		if trades == nil {
			trades = []models.Trade{}
		}
		writeJSON(w, http.StatusOK, trades)
		return
	}

	trades, err := h.store.GetRecentTrades(limit)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to fetch trades")
		log.Printf("[api] GetRecentTrades error: %v", err)
		return
	}
	if trades == nil {
		trades = []models.Trade{}
	}
	writeJSON(w, http.StatusOK, trades)
}

// GetFundingHistory returns recent funding rate updates.
//
//	GET /api/v1/funding?limit=20
func (h *Handlers) GetFundingHistory(w http.ResponseWriter, r *http.Request) {
	limit := queryInt(r, "limit", 50)

	history, err := h.store.GetFundingHistory(limit)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to fetch funding history")
		log.Printf("[api] GetFundingHistory error: %v", err)
		return
	}
	if history == nil {
		history = []models.FundingUpdate{}
	}
	writeJSON(w, http.StatusOK, history)
}

// GetLiquidations returns recent liquidation events.
//
//	GET /api/v1/liquidations?limit=20
func (h *Handlers) GetLiquidations(w http.ResponseWriter, r *http.Request) {
	limit := queryInt(r, "limit", 50)

	events, err := h.store.GetRecentLiquidations(limit)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to fetch liquidations")
		log.Printf("[api] GetRecentLiquidations error: %v", err)
		return
	}
	if events == nil {
		events = []models.LiquidationEvent{}
	}
	writeJSON(w, http.StatusOK, events)
}

// GetStats returns aggregate platform statistics.
//
//	GET /api/v1/stats
func (h *Handlers) GetStats(w http.ResponseWriter, r *http.Request) {
	openCount, err := h.store.GetOpenPositionCount()
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to fetch stats")
		log.Printf("[api] GetOpenPositionCount error: %v", err)
		return
	}

	volume, trades24h, err := h.store.GetVolume24h()
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to fetch stats")
		log.Printf("[api] GetVolume24h error: %v", err)
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"open_positions": openCount,
		"volume_24h":     volume,
		"trades_24h":     trades24h,
	})
}

// GetMarketStats returns market data for a specific asset.
//
//	GET /api/v1/market/0
func (h *Handlers) GetMarketStats(w http.ResponseWriter, r *http.Request) {
	assetStr := r.PathValue("asset")
	asset, err := strconv.ParseUint(assetStr, 10, 32)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid asset id")
		return
	}

	long, short, err := h.store.GetOpenInterest(uint32(asset))
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to fetch market stats")
		log.Printf("[api] GetOpenInterest error: %v", err)
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"asset":    asset,
		"oi_long":  long,
		"oi_short": short,
	})
}
