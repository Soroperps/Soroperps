package api

import (
	"encoding/json"
	"log"
	"net/http"
	"sync"

	"golang.org/x/net/websocket"
)

// WSMessage is a message sent to WebSocket clients.
type WSMessage struct {
	Type string      `json:"type"` // "price", "position", "trade", "liquidation", "funding"
	Data interface{} `json:"data"`
}

// Hub manages WebSocket connections and broadcasts messages.
type Hub struct {
	mu      sync.RWMutex
	clients map[*websocket.Conn]struct{}
}

// NewHub creates a new WebSocket hub.
func NewHub() *Hub {
	return &Hub{
		clients: make(map[*websocket.Conn]struct{}),
	}
}

// Handler returns an http.Handler for WebSocket upgrades.
//
//	GET /ws
func (hub *Hub) Handler() http.Handler {
	return websocket.Handler(func(ws *websocket.Conn) {
		hub.addClient(ws)
		defer hub.removeClient(ws)

		log.Printf("[ws] client connected: %s", ws.Request().RemoteAddr)

		// Keep connection alive — read and discard client messages
		buf := make([]byte, 512)
		for {
			_, err := ws.Read(buf)
			if err != nil {
				break
			}
		}

		log.Printf("[ws] client disconnected: %s", ws.Request().RemoteAddr)
	})
}

func (hub *Hub) addClient(ws *websocket.Conn) {
	hub.mu.Lock()
	defer hub.mu.Unlock()
	hub.clients[ws] = struct{}{}
}

func (hub *Hub) removeClient(ws *websocket.Conn) {
	hub.mu.Lock()
	defer hub.mu.Unlock()
	delete(hub.clients, ws)
	ws.Close()
}

// Broadcast sends a message to all connected WebSocket clients.
func (hub *Hub) Broadcast(msgType string, data interface{}) {
	msg := WSMessage{Type: msgType, Data: data}
	payload, err := json.Marshal(msg)
	if err != nil {
		log.Printf("[ws] marshal error: %v", err)
		return
	}

	hub.mu.RLock()
	clients := make([]*websocket.Conn, 0, len(hub.clients))
	for ws := range hub.clients {
		clients = append(clients, ws)
	}
	hub.mu.RUnlock()

	for _, ws := range clients {
		if _, err := ws.Write(payload); err != nil {
			hub.removeClient(ws)
		}
	}
}

// ClientCount returns the number of connected clients.
func (hub *Hub) ClientCount() int {
	hub.mu.RLock()
	defer hub.mu.RUnlock()
	return len(hub.clients)
}
