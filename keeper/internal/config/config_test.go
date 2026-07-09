package config

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestLoadConfig(t *testing.T) {
	content := `{
		"rpc_url": "https://soroban-testnet.stellar.org",
		"network_passphrase": "Test SDF Network ; September 2015",
		"secret_key": "STEST123",
		"keeper_address": "GTEST456",
		"position_manager_contract_id": "CTEST789",
		"liquidation_interval_sec": 15,
		"funding_interval_sec": 7200,
		"scan_interval_sec": 3
	}`

	tmpDir := t.TempDir()
	path := filepath.Join(tmpDir, "config.json")
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load() error: %v", err)
	}

	if cfg.RPCURL != "https://soroban-testnet.stellar.org" {
		t.Errorf("RPCURL = %q, want %q", cfg.RPCURL, "https://soroban-testnet.stellar.org")
	}
	if cfg.SecretKey != "STEST123" {
		t.Errorf("SecretKey = %q, want %q", cfg.SecretKey, "STEST123")
	}
	if cfg.LiquidationInterval() != 15*time.Second {
		t.Errorf("LiquidationInterval() = %v, want 15s", cfg.LiquidationInterval())
	}
	if cfg.FundingInterval() != 7200*time.Second {
		t.Errorf("FundingInterval() = %v, want 7200s", cfg.FundingInterval())
	}
	if cfg.ScanInterval() != 3*time.Second {
		t.Errorf("ScanInterval() = %v, want 3s", cfg.ScanInterval())
	}
}

func TestLoadConfigDefaults(t *testing.T) {
	content := `{
		"rpc_url": "http://localhost:8000",
		"secret_key": "STEST",
		"position_manager_contract_id": "CTEST"
	}`

	tmpDir := t.TempDir()
	path := filepath.Join(tmpDir, "config.json")
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load() error: %v", err)
	}

	// Check defaults
	if cfg.LiquidationInterval() != 10*time.Second {
		t.Errorf("default LiquidationInterval() = %v, want 10s", cfg.LiquidationInterval())
	}
	if cfg.FundingInterval() != 3600*time.Second {
		t.Errorf("default FundingInterval() = %v, want 3600s", cfg.FundingInterval())
	}
	if cfg.ScanInterval() != 5*time.Second {
		t.Errorf("default ScanInterval() = %v, want 5s", cfg.ScanInterval())
	}
}

func TestLoadConfigMissingRequired(t *testing.T) {
	tests := []struct {
		name    string
		content string
	}{
		{
			name:    "missing rpc_url",
			content: `{"secret_key": "S", "position_manager_contract_id": "C"}`,
		},
		{
			name:    "missing secret_key",
			content: `{"rpc_url": "http://x", "position_manager_contract_id": "C"}`,
		},
		{
			name:    "missing position_manager",
			content: `{"rpc_url": "http://x", "secret_key": "S"}`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tmpDir := t.TempDir()
			path := filepath.Join(tmpDir, "config.json")
			if err := os.WriteFile(path, []byte(tt.content), 0644); err != nil {
				t.Fatal(err)
			}

			_, err := Load(path)
			if err == nil {
				t.Error("Load() should have returned error for missing required field")
			}
		})
	}
}

func TestLoadConfigFileNotFound(t *testing.T) {
	_, err := Load("/nonexistent/config.json")
	if err == nil {
		t.Error("Load() should have returned error for missing file")
	}
}

func TestContractIDs(t *testing.T) {
	cfg := &Config{
		VaultID:             "V",
		PositionManagerID:   "PM",
		OracleAdapterID:     "O",
		LiquidationEngineID: "LE",
		FundingRateID:       "FR",
	}

	ids := cfg.ContractIDs()
	if len(ids) != 5 {
		t.Errorf("ContractIDs() len = %d, want 5", len(ids))
	}
	if ids[0] != "V" || ids[1] != "PM" || ids[2] != "O" || ids[3] != "LE" || ids[4] != "FR" {
		t.Errorf("ContractIDs() = %v, unexpected values", ids)
	}
}
