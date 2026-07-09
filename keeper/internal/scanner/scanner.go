package scanner

import (
	"context"
	"log"
	"sync"
	"time"

	"github.com/soroperps/keeper/internal/stellar"
)

// PositionInfo holds tracked position data parsed from events.
type PositionInfo struct {
	PositionID uint64
	Trader     string
	Asset      uint32
	Direction  string // "long" or "short"
	Size       int64
	Collateral int64
	EntryPrice int64
}

// Scanner monitors contract events to track open positions.
type Scanner struct {
	client      *stellar.Client
	contractIDs []string
	interval    time.Duration

	mu            sync.RWMutex
	openPositions map[string][]PositionInfo // trader address -> positions
	lastLedger    int64
}

// New creates a new Scanner.
func New(client *stellar.Client, contractIDs []string, interval time.Duration) *Scanner {
	return &Scanner{
		client:        client,
		contractIDs:   contractIDs,
		interval:      interval,
		openPositions: make(map[string][]PositionInfo),
	}
}

// Start begins scanning for events. Blocks until context is cancelled.
func (s *Scanner) Start(ctx context.Context) {
	log.Println("[scanner] starting event scanner")

	// Initial scan to catch up on recent events
	s.scan()

	ticker := time.NewTicker(s.interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			s.scan()
		case <-ctx.Done():
			log.Println("[scanner] shutting down")
			return
		}
	}
}

// GetOpenPositions returns a snapshot of all tracked open positions.
func (s *Scanner) GetOpenPositions() map[string][]PositionInfo {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Deep copy to avoid race conditions
	result := make(map[string][]PositionInfo, len(s.openPositions))
	for k, v := range s.openPositions {
		positions := make([]PositionInfo, len(v))
		copy(positions, v)
		result[k] = positions
	}
	return result
}

// GetPositionCount returns the total number of tracked open positions.
func (s *Scanner) GetPositionCount() int {
	s.mu.RLock()
	defer s.mu.RUnlock()

	count := 0
	for _, positions := range s.openPositions {
		count += len(positions)
	}
	return count
}

func (s *Scanner) scan() {
	startLedger := s.lastLedger
	if startLedger == 0 {
		// On first scan, start from a recent ledger.
		// In production, use getLatestLedger() - some_offset
		startLedger = 1
	}

	result, err := s.client.GetEvents(startLedger, s.contractIDs, 1000)
	if err != nil {
		log.Printf("[scanner] error fetching events: %v", err)
		return
	}

	for _, event := range result.Events {
		s.processEvent(event)
	}

	if result.LatestLedger > s.lastLedger {
		s.lastLedger = result.LatestLedger
	}
}

func (s *Scanner) processEvent(event stellar.EventInfo) {
	// Parse event topics to determine type.
	// Topics are XDR-encoded ScVal values.
	// Topic[0] is typically the event name symbol.
	//
	// For position_opened events:
	//   topics: ["position_opened", trader_address, position_id]
	//   value: (direction, size, collateral, entry_price, leverage, fee)
	//
	// For position_closed / liquidation events:
	//   topics: ["position_closed", trader_address, position_id]
	//   or: ["liquidation", trader_address, position_id]
	//
	// In production, we'd decode the XDR. For now, we log and track.

	if len(event.Topic) == 0 {
		return
	}

	// The topic[0] is an XDR-encoded Symbol. We'd need to decode it
	// to match against "position_opened", "position_closed", "liquidation".
	// For the MVP structure, we show the flow:

	log.Printf("[scanner] event on contract %s, ledger %d, topics: %v",
		event.ContractID, event.Ledger, event.Topic)
}

// AddPosition manually adds a position to track (for testing).
func (s *Scanner) AddPosition(trader string, pos PositionInfo) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.openPositions[trader] = append(s.openPositions[trader], pos)
}

// RemovePosition removes a tracked position (for testing or after liquidation).
func (s *Scanner) RemovePosition(trader string, positionID uint64) {
	s.mu.Lock()
	defer s.mu.Unlock()

	positions := s.openPositions[trader]
	for i, p := range positions {
		if p.PositionID == positionID {
			s.openPositions[trader] = append(positions[:i], positions[i+1:]...)
			if len(s.openPositions[trader]) == 0 {
				delete(s.openPositions, trader)
			}
			return
		}
	}
}
