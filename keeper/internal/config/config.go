package config

import (
	"encoding/json"
	"fmt"
	"os"
	"time"
)

// Config holds all keeper configuration.
type Config struct {
	// Stellar network
	RPCURL            string `json:"rpc_url"`
	NetworkPassphrase string `json:"network_passphrase"`
	SecretKey         string `json:"secret_key"`
	KeeperAddress     string `json:"keeper_address"`

	// Contract IDs
	VaultID             string `json:"vault_contract_id"`
	PositionManagerID   string `json:"position_manager_contract_id"`
	OracleAdapterID     string `json:"oracle_adapter_contract_id"`
	LiquidationEngineID string `json:"liquidation_engine_contract_id"`
	FundingRateID       string `json:"funding_rate_contract_id"`

	// Intervals
	LiquidationIntervalSec int `json:"liquidation_interval_sec"`
	FundingIntervalSec     int `json:"funding_interval_sec"`
	ScanIntervalSec        int `json:"scan_interval_sec"`
}

// LiquidationInterval returns the liquidation check interval as a Duration.
func (c *Config) LiquidationInterval() time.Duration {
	if c.LiquidationIntervalSec <= 0 {
		return 10 * time.Second
	}
	return time.Duration(c.LiquidationIntervalSec) * time.Second
}

// FundingInterval returns the funding trigger interval as a Duration.
func (c *Config) FundingInterval() time.Duration {
	if c.FundingIntervalSec <= 0 {
		return 3600 * time.Second
	}
	return time.Duration(c.FundingIntervalSec) * time.Second
}

// ScanInterval returns the event scan interval as a Duration.
func (c *Config) ScanInterval() time.Duration {
	if c.ScanIntervalSec <= 0 {
		return 5 * time.Second
	}
	return time.Duration(c.ScanIntervalSec) * time.Second
}

// ContractIDs returns all contract IDs for event scanning.
func (c *Config) ContractIDs() []string {
	return []string{
		c.VaultID,
		c.PositionManagerID,
		c.OracleAdapterID,
		c.LiquidationEngineID,
		c.FundingRateID,
	}
}

// Load reads a JSON config file from the given path.
func Load(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read config %s: %w", path, err)
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("parse config %s: %w", path, err)
	}

	if cfg.RPCURL == "" {
		return nil, fmt.Errorf("rpc_url is required")
	}
	if cfg.SecretKey == "" {
		return nil, fmt.Errorf("secret_key is required")
	}
	if cfg.PositionManagerID == "" {
		return nil, fmt.Errorf("position_manager_contract_id is required")
	}

	return &cfg, nil
}
