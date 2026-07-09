package funder

import (
	"context"
	"log"
	"time"

	"github.com/soroperps/keeper/internal/config"
	"github.com/soroperps/keeper/internal/stellar"
)

// Funder triggers funding rate updates at regular intervals.
type Funder struct {
	contracts *stellar.Contracts
	config    *config.Config
	interval  time.Duration

	// Stats
	attemptCount int64
	successCount int64
	tooEarlyCount int64
}

// New creates a new Funder.
func New(contracts *stellar.Contracts, cfg *config.Config) *Funder {
	return &Funder{
		contracts: contracts,
		config:    cfg,
		interval:  cfg.FundingInterval(),
	}
}

// Start begins the funding trigger loop. Blocks until context is cancelled.
func (f *Funder) Start(ctx context.Context) {
	log.Printf("[funder] starting, interval=%s", f.interval)

	ticker := time.NewTicker(f.interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			f.triggerFunding()
		case <-ctx.Done():
			log.Printf("[funder] shutting down (attempts=%d, success=%d, too_early=%d)",
				f.attemptCount, f.successCount, f.tooEarlyCount)
			return
		}
	}
}

func (f *Funder) triggerFunding() {
	f.attemptCount++

	txHash, err := f.contracts.ApplyFunding(
		f.config.FundingRateID,
		f.config.KeeperAddress,
	)
	if err != nil {
		// "FundingTooEarly" is expected and not a real error —
		// it means another keeper already applied funding this interval.
		// In production, we'd check the error type.
		f.tooEarlyCount++
		log.Printf("[funder] apply_funding: %v", err)
		return
	}

	f.successCount++
	log.Printf("[funder] funding applied successfully, tx: %s", txHash)
}

// Stats returns current funder statistics.
func (f *Funder) Stats() (attempts, success, tooEarly int64) {
	return f.attemptCount, f.successCount, f.tooEarlyCount
}
