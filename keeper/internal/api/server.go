package api

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"time"

	"github.com/soroperps/keeper/internal/store"
)

// Server is the HTTP/WebSocket API server.
type Server struct {
	httpServer *http.Server
	handlers   *Handlers
	Hub        *Hub
}

// NewServer creates a new API server.
func NewServer(addr string, s *store.Store) *Server {
	hub := NewHub()
	handlers := NewHandlers(s)

	mux := http.NewServeMux()
	handlers.RegisterRoutes(mux)
	mux.Handle("/ws", hub.Handler())

	return &Server{
		httpServer: &http.Server{
			Addr:         addr,
			Handler:      CORS(mux),
			ReadTimeout:  10 * time.Second,
			WriteTimeout: 30 * time.Second,
			IdleTimeout:  60 * time.Second,
		},
		handlers: handlers,
		Hub:      hub,
	}
}

// Start begins listening. Blocks until context is cancelled.
func (s *Server) Start(ctx context.Context) {
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err := s.httpServer.Shutdown(shutdownCtx); err != nil {
			log.Printf("[api] shutdown error: %v", err)
		}
	}()

	log.Printf("[api] listening on %s", s.httpServer.Addr)
	if err := s.httpServer.ListenAndServe(); err != http.ErrServerClosed {
		log.Printf("[api] server error: %v", err)
	}
	log.Println("[api] server stopped")
}

// Addr returns the server's listen address.
func (s *Server) Addr() string {
	return fmt.Sprintf("http://%s", s.httpServer.Addr)
}
