package main

import (
	"context"
	"flag"
	"log"
	"os"
	"os/signal"
	"sync"
	"syscall"

	"github.com/soroperps/keeper/internal/api"
	"github.com/soroperps/keeper/internal/config"
	"github.com/soroperps/keeper/internal/indexer"
	"github.com/soroperps/keeper/internal/stellar"
	"github.com/soroperps/keeper/internal/store"
)

func main() {
	configPath := flag.String("config", "deployments/testnet.json", "path to config file")
	listenAddr := flag.String("addr", ":8080", "API listen address")
	dbPath := flag.String("db", "soroperps.db", "SQLite database path")
	flag.Parse()

	log.SetFlags(log.LstdFlags | log.Lshortfile)
	log.Println("SoroPerps API server starting...")

	// Load config
	cfg, err := config.Load(*configPath)
	if err != nil {
		log.Fatalf("failed to load config: %v", err)
	}
	log.Printf("config loaded: rpc=%s", cfg.RPCURL)

	// Open database
	db, err := store.New(*dbPath)
	if err != nil {
		log.Fatalf("failed to open database: %v", err)
	}
	defer db.Close()
	log.Printf("database opened: %s", *dbPath)

	// Create Stellar client
	client := stellar.NewClient(cfg.RPCURL)

	// Health check
	health, err := client.GetHealth()
	if err != nil {
		log.Fatalf("RPC health check failed: %v", err)
	}
	log.Printf("RPC healthy: %s", health.Status)

	// Graceful shutdown
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	var wg sync.WaitGroup

	// Start event indexer
	idx := indexer.New(client, db, cfg.ContractIDs(), cfg.ScanInterval())
	wg.Add(1)
	go func() {
		defer wg.Done()
		idx.Start(ctx)
	}()

	// Start API server
	server := api.NewServer(*listenAddr, db)
	wg.Add(1)
	go func() {
		defer wg.Done()
		server.Start(ctx)
	}()

	log.Printf("API server: http://localhost%s", *listenAddr)
	log.Printf("WebSocket:  ws://localhost%s/ws", *listenAddr)
	log.Println("endpoints:")
	log.Println("  GET /api/v1/health")
	log.Println("  GET /api/v1/positions?trader=&status=open")
	log.Println("  GET /api/v1/positions/{id}")
	log.Println("  GET /api/v1/trades?trader=&limit=50")
	log.Println("  GET /api/v1/funding?limit=50")
	log.Println("  GET /api/v1/liquidations?limit=50")
	log.Println("  GET /api/v1/stats")
	log.Println("  GET /api/v1/market/{asset}")
	log.Println("  WS  /ws")
	log.Println("")
	log.Println("all components started, waiting for shutdown signal...")

	wg.Wait()
	log.Println("API server shut down cleanly")
}
