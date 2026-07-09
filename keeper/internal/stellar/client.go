package stellar

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Client wraps the Soroban JSON-RPC endpoint.
type Client struct {
	rpcURL     string
	httpClient *http.Client
	requestID  int
}

// NewClient creates a new Soroban RPC client.
func NewClient(rpcURL string) *Client {
	return &Client{
		rpcURL: rpcURL,
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
	}
}

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

type rpcRequest struct {
	JSONRPC string      `json:"jsonrpc"`
	ID      int         `json:"id"`
	Method  string      `json:"method"`
	Params  interface{} `json:"params,omitempty"`
}

type rpcResponse struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      int             `json:"id"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   *rpcError       `json:"error,omitempty"`
}

type rpcError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

func (e *rpcError) Error() string {
	return fmt.Sprintf("rpc error %d: %s", e.Code, e.Message)
}

// ---------------------------------------------------------------------------
// Soroban-specific types
// ---------------------------------------------------------------------------

// SimulateTransactionParams holds params for simulateTransaction.
type SimulateTransactionParams struct {
	Transaction string `json:"transaction"`
}

// SimulateResult is the response from simulateTransaction.
type SimulateResult struct {
	TransactionData string           `json:"transactionData"`
	MinResourceFee  string           `json:"minResourceFee"`
	Results         []SimResultEntry `json:"results,omitempty"`
	Error           string           `json:"error,omitempty"`
	LatestLedger    int64            `json:"latestLedger"`
}

// SimResultEntry holds a single invocation result.
type SimResultEntry struct {
	XDR  string `json:"xdr"`
	Auth []string `json:"auth,omitempty"`
}

// SendTransactionParams holds params for sendTransaction.
type SendTransactionParams struct {
	Transaction string `json:"transaction"`
}

// SendTransactionResult is the response from sendTransaction.
type SendTransactionResult struct {
	Status string `json:"status"`
	Hash   string `json:"hash"`
	Error  string `json:"errorResultXdr,omitempty"`
}

// GetTransactionParams holds params for getTransaction.
type GetTransactionParams struct {
	Hash string `json:"hash"`
}

// GetTransactionResult is the response from getTransaction.
type GetTransactionResult struct {
	Status      string `json:"status"`
	LatestLedger int64 `json:"latestLedger"`
	ResultXDR   string `json:"resultXdr,omitempty"`
	EnvelopeXDR string `json:"envelopeXdr,omitempty"`
}

// EventFilter defines filters for getEvents.
type EventFilter struct {
	Type       string   `json:"type,omitempty"`
	ContractIDs []string `json:"contractIds,omitempty"`
}

// GetEventsParams holds params for getEvents.
type GetEventsParams struct {
	StartLedger int64         `json:"startLedger"`
	Filters     []EventFilter `json:"filters,omitempty"`
	Pagination  *Pagination   `json:"pagination,omitempty"`
}

// Pagination controls event pagination.
type Pagination struct {
	Limit  int    `json:"limit,omitempty"`
	Cursor string `json:"cursor,omitempty"`
}

// GetEventsResult is the response from getEvents.
type GetEventsResult struct {
	Events       []EventInfo `json:"events"`
	LatestLedger int64       `json:"latestLedger"`
}

// EventInfo represents a single event from getEvents.
type EventInfo struct {
	Type         string   `json:"type"`
	Ledger       int64    `json:"ledger"`
	LedgerClosedAt string `json:"ledgerClosedAt"`
	ContractID   string   `json:"contractId"`
	ID           string   `json:"id"`
	PagingToken  string   `json:"pagingToken"`
	Topic        []string `json:"topic"`
	Value        string   `json:"value"`
}

// HealthResult is the response from getHealth.
type HealthResult struct {
	Status string `json:"status"`
}

// ---------------------------------------------------------------------------
// RPC methods
// ---------------------------------------------------------------------------

func (c *Client) call(method string, params interface{}) (json.RawMessage, error) {
	c.requestID++

	req := rpcRequest{
		JSONRPC: "2.0",
		ID:      c.requestID,
		Method:  method,
		Params:  params,
	}

	body, err := json.Marshal(req)
	if err != nil {
		return nil, fmt.Errorf("marshal request: %w", err)
	}

	resp, err := c.httpClient.Post(c.rpcURL, "application/json", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("rpc call %s: %w", method, err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("read response: %w", err)
	}

	var rpcResp rpcResponse
	if err := json.Unmarshal(respBody, &rpcResp); err != nil {
		return nil, fmt.Errorf("unmarshal response: %w", err)
	}

	if rpcResp.Error != nil {
		return nil, rpcResp.Error
	}

	return rpcResp.Result, nil
}

// GetHealth checks if the RPC server is healthy.
func (c *Client) GetHealth() (*HealthResult, error) {
	raw, err := c.call("getHealth", nil)
	if err != nil {
		return nil, err
	}
	var result HealthResult
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, err
	}
	return &result, nil
}

// SimulateTransaction simulates a transaction without submitting.
func (c *Client) SimulateTransaction(txXDR string) (*SimulateResult, error) {
	params := SimulateTransactionParams{Transaction: txXDR}
	raw, err := c.call("simulateTransaction", params)
	if err != nil {
		return nil, err
	}
	var result SimulateResult
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, err
	}
	return &result, nil
}

// SendTransaction submits a signed transaction.
func (c *Client) SendTransaction(txXDR string) (*SendTransactionResult, error) {
	params := SendTransactionParams{Transaction: txXDR}
	raw, err := c.call("sendTransaction", params)
	if err != nil {
		return nil, err
	}
	var result SendTransactionResult
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, err
	}
	return &result, nil
}

// GetTransaction gets the result of a submitted transaction.
func (c *Client) GetTransaction(hash string) (*GetTransactionResult, error) {
	params := GetTransactionParams{Hash: hash}
	raw, err := c.call("getTransaction", params)
	if err != nil {
		return nil, err
	}
	var result GetTransactionResult
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, err
	}
	return &result, nil
}

// GetEvents fetches contract events from the RPC.
func (c *Client) GetEvents(startLedger int64, contractIDs []string, limit int) (*GetEventsResult, error) {
	params := GetEventsParams{
		StartLedger: startLedger,
		Filters: []EventFilter{
			{
				Type:        "contract",
				ContractIDs: contractIDs,
			},
		},
		Pagination: &Pagination{
			Limit: limit,
		},
	}

	raw, err := c.call("getEvents", params)
	if err != nil {
		return nil, err
	}
	var result GetEventsResult
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, err
	}
	return &result, nil
}
