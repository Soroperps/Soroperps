package liquidator

import (
	"context"
	"log"
	"time"

	"github.com/soroperps/keeper/internal/config"
	"github.com/soroperps/keeper/internal/scanner"
	"github.com/soroperps/keeper/internal/stellar"
)

// Liquidator monitors positions and triggers liquidations when needed.
type Liquidator struct {
	contracts *stellar.Contracts
	scanner   *scanner.Scanner
	config    *config.Config
	interval  time.Duration

	// Stats
	checksPerformed   int64
	liquidationsFound int64
	liquidationsSent  int64
}

// New creates a new Liquidator.
func New(contracts *stellar.Contracts, scan *scanner.Scanner, cfg *config.Config) *Liquidator {
	return &Liquidator{
		contracts: contracts,
		scanner:   scan,
		config:    cfg,
		interval:  cfg.LiquidationInterval(),
	}
}

// Start begins the liquidation monitoring loop. Blocks until context is cancelled.
func (l *Liquidator) Start(ctx context.Context) {
	log.Printf("[liquidator] starting, interval=%s", l.interval)

	ticker := time.NewTicker(l.interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			l.checkAndLiquidate()
		case <-ctx.Done():
			log.Printf("[liquidator] shutting down (checks=%d, found=%d, sent=%d)",
				l.checksPerformed, l.liquidationsFound, l.liquidationsSent)
			return
		}
	}
}

func (l *Liquidator) checkAndLiquidate() {
	positions := l.scanner.GetOpenPositions()

	totalPositions := 0
	for _, poss := range positions {
		totalPositions += len(poss)
	}

	if totalPositions == 0 {
		return
	}

	log.Printf("[liquidator] checking %d positions across %d traders",
		totalPositions, len(positions))

	for trader, poss := range positions {
		for _, pos := range poss {
			l.checksPerformed++
			l.checkPosition(trader, pos)
		}
	}
}

func (l *Liquidator) checkPosition(trader string, pos scanner.PositionInfo) {
	// Step 1: Simulate is_liquidatable (free, no gas)
	liquidatable, err := l.contracts.IsLiquidatable(
		l.config.LiquidationEngineID,
		trader,
		pos.PositionID,
		pos.Asset,
	)
	if err != nil {
		// Expected during development — XDR encoding not yet implemented
		// In production, this would be a real error worth logging
		return
	}

	if !liquidatable {
		return
	}

	l.liquidationsFound++
	log.Printf("[liquidator] position %d for trader %s is liquidatable! Submitting tx...",
		pos.PositionID, trader)

	// Step 2: Submit liquidation transaction
	txHash, err := l.contracts.Liquidate(
		l.config.LiquidationEngineID,
		l.config.KeeperAddress,
		trader,
		pos.PositionID,
		pos.Asset,
	)
	if err != nil {
		log.Printf("[liquidator] failed to liquidate position %d: %v", pos.PositionID, err)
		return
	}

	l.liquidationsSent++
	log.Printf("[liquidator] liquidation tx submitted: %s", txHash)

	// Remove from tracked positions (will be re-added if event scan finds it still open)
	l.scanner.RemovePosition(trader, pos.PositionID)
}

// Stats returns current liquidator statistics.
func (l *Liquidator) Stats() (checks, found, sent int64) {
	return l.checksPerformed, l.liquidationsFound, l.liquidationsSent
}
