package indexer

import (
	"context"
	"log"
	"time"

	"github.com/soroperps/keeper/internal/models"
	"github.com/soroperps/keeper/internal/stellar"
	"github.com/soroperps/keeper/internal/store"
)

// Indexer polls Soroban events and writes them to the store.
type Indexer struct {
	client      *stellar.Client
	store       *store.Store
	contractIDs []string
	interval    time.Duration

	eventsProcessed int64
}

// New creates a new Indexer.
func New(client *stellar.Client, s *store.Store, contractIDs []string, interval time.Duration) *Indexer {
	return &Indexer{
		client:      client,
		store:       s,
		contractIDs: contractIDs,
		interval:    interval,
	}
}

// Start begins the indexing loop. Blocks until context is cancelled.
func (idx *Indexer) Start(ctx context.Context) {
	log.Println("[indexer] starting event indexer")

	// Initial index
	idx.index()

	ticker := time.NewTicker(idx.interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			idx.index()
		case <-ctx.Done():
			log.Printf("[indexer] shutting down (events_processed=%d)", idx.eventsProcessed)
			return
		}
	}
}

func (idx *Indexer) index() {
	lastLedger, _, err := idx.store.GetCursor()
	if err != nil {
		log.Printf("[indexer] error reading cursor: %v", err)
		return
	}

	startLedger := lastLedger
	if startLedger == 0 {
		startLedger = 1
	}

	result, err := idx.client.GetEvents(startLedger, idx.contractIDs, 1000)
	if err != nil {
		log.Printf("[indexer] error fetching events: %v", err)
		return
	}

	for _, event := range result.Events {
		idx.processEvent(event)
		idx.eventsProcessed++
	}

	if result.LatestLedger > lastLedger {
		cursor := ""
		if len(result.Events) > 0 {
			cursor = result.Events[len(result.Events)-1].PagingToken
		}
		if err := idx.store.SetCursor(result.LatestLedger, cursor); err != nil {
			log.Printf("[indexer] error updating cursor: %v", err)
		}
	}
}

func (idx *Indexer) processEvent(event stellar.EventInfo) {
	// Events are XDR-encoded. In production, decode the topic[0] Symbol
	// to determine event type (position_opened, position_closed, liquidation,
	// funding_applied, deposit, withdraw, etc.) and parse the value XDR
	// into the appropriate model struct.
	//
	// For now, log the event for debugging.
	if len(event.Topic) == 0 {
		return
	}

	log.Printf("[indexer] event contract=%s ledger=%d topics=%v",
		event.ContractID, event.Ledger, event.Topic)
}

// ProcessPositionOpened handles a decoded position_opened event.
// Called by the event decoder once XDR parsing is implemented.
func (idx *Indexer) ProcessPositionOpened(
	positionID uint64, trader string, asset uint32,
	direction string, size, collateral, entryPrice int64,
	leverage uint32, timestamp int64,
) error {
	pos := &models.Position{
		ID:         positionID,
		Trader:     trader,
		Asset:      asset,
		Direction:  direction,
		Size:       size,
		Collateral: collateral,
		EntryPrice: entryPrice,
		Leverage:   leverage,
		OpenedAt:   timestamp,
		Status:     "open",
	}
	return idx.store.UpsertPosition(pos)
}

// ProcessPositionClosed handles a decoded position_closed event.
func (idx *Indexer) ProcessPositionClosed(
	positionID uint64, trader string, asset uint32,
	direction string, size, entryPrice, exitPrice, pnl, fee, timestamp int64,
) error {
	// Update position
	closedAt := timestamp
	pos := &models.Position{
		ID:          positionID,
		Trader:      trader,
		Asset:       asset,
		Direction:   direction,
		Size:        size,
		Collateral:  0, // will be preserved by upsert
		EntryPrice:  entryPrice,
		Leverage:    0,
		OpenedAt:    0,
		Status:      "closed",
		ClosedAt:    &closedAt,
		ExitPrice:   &exitPrice,
		RealizedPnl: &pnl,
		CloseFee:    &fee,
	}
	if err := idx.store.UpsertPosition(pos); err != nil {
		return err
	}

	// Record trade
	trade := &models.Trade{
		PositionID:  positionID,
		Trader:      trader,
		Asset:       asset,
		Direction:   direction,
		Size:        size,
		EntryPrice:  entryPrice,
		ExitPrice:   exitPrice,
		RealizedPnl: pnl,
		Fee:         fee,
		Type:        "close",
		Timestamp:   timestamp,
	}
	return idx.store.InsertTrade(trade)
}

// ProcessLiquidation handles a decoded liquidation event.
func (idx *Indexer) ProcessLiquidation(
	positionID uint64, trader, keeper string, asset uint32,
	markPrice, marginRatio, penalty, keeperReward, timestamp int64,
) error {
	liq := &models.LiquidationEvent{
		PositionID:   positionID,
		Trader:       trader,
		Keeper:       keeper,
		Asset:        asset,
		MarkPrice:    markPrice,
		MarginRatio:  marginRatio,
		Penalty:      penalty,
		KeeperReward: keeperReward,
		Timestamp:    timestamp,
	}
	return idx.store.InsertLiquidation(liq)
}

// ProcessFundingApplied handles a decoded funding_applied event.
func (idx *Indexer) ProcessFundingApplied(
	longRate, shortRate, cumLong, cumShort, oiLong, oiShort, timestamp int64,
) error {
	f := &models.FundingUpdate{
		LongRate:        longRate,
		ShortRate:       shortRate,
		CumulativeLong:  cumLong,
		CumulativeShort: cumShort,
		OILong:          oiLong,
		OIShort:         oiShort,
		Timestamp:       timestamp,
	}
	return idx.store.InsertFundingUpdate(f)
}

// Stats returns the number of events processed.
func (idx *Indexer) Stats() int64 {
	return idx.eventsProcessed
}
