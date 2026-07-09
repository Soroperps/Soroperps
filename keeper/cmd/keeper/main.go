package main

import (
	"context"
	"flag"
	"log"
	"os"
	"os/signal"
	"sync"
	"syscall"

	"github.com/soroperps/keeper/internal/config"
	"github.com/soroperps/keeper/internal/funder"
	"github.com/soroperps/keeper/internal/liquidator"
	"github.com/soroperps/keeper/internal/scanner"
	"github.com/soroperps/keeper/internal/stellar"
)

func main() {
	configPath := flag.String("config", "deployments/testnet.json", "path to config file")
	flag.Parse()

	log.SetFlags(log.LstdFlags | log.Lshortfile)
	log.Println("SoroPerps Keeper starting...")

	// Load config
	cfg, err := config.Load(*configPath)
	if err != nil {
		log.Fatalf("failed to load config: %v", err)
	}
	log.Printf("config loaded: rpc=%s, position_manager=%s", cfg.RPCURL, cfg.PositionManagerID)

	// Create Stellar client
	client := stellar.NewClient(cfg.RPCURL)

	// Health check
	health, err := client.GetHealth()
	if err != nil {
		log.Fatalf("RPC health check failed: %v", err)
	}
	log.Printf("RPC healthy: %s", health.Status)

	// Create contract invoker
	contracts := stellar.NewContracts(client, cfg.NetworkPassphrase, cfg.SecretKey)

	// Create components
	scan := scanner.New(client, cfg.ContractIDs(), cfg.ScanInterval())
	liq := liquidator.New(contracts, scan, cfg)
	fund := funder.New(contracts, cfg)

	// Graceful shutdown
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	// Start all components
	var wg sync.WaitGroup

	wg.Add(1)
	go func() {
		defer wg.Done()
		scan.Start(ctx)
	}()

	wg.Add(1)
	go func() {
		defer wg.Done()
		liq.Start(ctx)
	}()

	wg.Add(1)
	go func() {
		defer wg.Done()
		fund.Start(ctx)
	}()

	log.Println("all components started, waiting for shutdown signal...")
	wg.Wait()
	log.Println("keeper shut down cleanly")
}
