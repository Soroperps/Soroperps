# SoroPerps

Pool-based perpetual futures DEX on [Stellar/Soroban](https://soroban.stellar.org). LPs deposit USDC into a vault that acts as counterparty to all trader PnL. Traders open leveraged long/short positions priced by [Reflector](https://reflector.network) oracle. Liquidation and funding rates are enforced by on-chain contracts and off-chain Go keepers.

Backend + contracts only (no frontend).

## Architecture

```
soroperps/
  contracts/
    perps-types/           # Shared types, errors, constants (library, not a contract)
    vault/                 # LP vault: deposit/withdraw USDC, mint/burn shares
    oracle-adapter/        # Wraps Reflector oracle with price caching + staleness checks
    position-manager/      # Open/close leveraged positions, PnL settlement, fees
    liquidation-engine/    # Health checks + force-close underwater positions
    funding-rate/          # Skew-based funding rate mechanism
    integration-tests/     # Cross-contract integration tests
  keeper/
    cmd/keeper/            # Liquidation + funding keeper bot
    cmd/api/               # REST + WebSocket API server
    internal/
      api/                 # HTTP handlers, WebSocket hub, CORS middleware
      config/              # JSON config loader
      funder/              # Funding rate trigger bot
      indexer/             # Soroban event indexer → SQLite
      liquidator/          # Liquidation scanner bot
      models/              # Domain types
      scanner/             # Position event scanner
      stellar/             # Stellar RPC client + contract invocation builders
      store/               # SQLite persistence layer
  scripts/
    deploy.sh              # Build, deploy, initialize all contracts on testnet
    setup-testnet.sh       # Create and fund test accounts
```

## How It Works

**LPs** deposit USDC into the vault and receive shares proportional to pool value. The vault acts as the counterparty to all trades — when traders lose, LPs profit; when traders win, LPs pay.

**Traders** open leveraged positions (1x–50x) by posting USDC collateral. Positions are priced against the Reflector oracle. On close, PnL is settled atomically: the vault unlocks liquidity, adjusts pool accounting, collects fees, and returns funds to the trader in a single call.

**Keepers** are off-chain Go services that:
- Scan for underwater positions and submit liquidation transactions
- Trigger funding rate updates at regular intervals
- Index on-chain events into SQLite for the API server

**Funding rates** are skew-based: when long open interest exceeds short, longs pay shorts (and vice versa). This incentivizes balanced markets. A protocol spread is taken from both sides.

## Key Design Decisions

- All monetary values use `i128` with 7 decimal places (Stellar native precision)
- Positions stored as individual Persistent entries keyed by `(trader, position_id)` — O(1) per-tx cost, no DoS vector
- PnL losses are capped at collateral to prevent vault accounting inflation
- Lazy funding settlement via cumulative indices, settled per-position at close
- Oracle adapter indirection allows swapping price providers without redeploying
- Asset ID stored in each position to prevent cross-asset PnL manipulation

## Prerequisites

- [Rust](https://rustup.rs) (see `rust-toolchain.toml`)
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) (`stellar`)
- [Go 1.25+](https://go.dev/dl/)
- GCC (for `go-sqlite3` CGo compilation)

## Build

### Contracts

```bash
stellar contract build
```

All contracts produce <64KB WASM optimized for on-chain deployment.

### Keeper + API Server

```bash
cd keeper
go build ./cmd/keeper
go build ./cmd/api
```

## Test

### Contract Tests (94 tests)

```bash
cargo test
```

This runs:
- 17 vault unit tests
- 8 oracle adapter unit tests
- 12 position manager unit tests
- 9 liquidation engine unit tests
- 8 funding rate unit tests
- 40 integration tests (lifecycle, access control, edge cases)

### Go Tests

```bash
cd keeper
go test ./...
```

## Deploy to Testnet

```bash
# 1. Create and fund test accounts
./scripts/setup-testnet.sh

# 2. Build, deploy, and initialize all contracts
./scripts/deploy.sh
```

The deploy script will output a JSON config file at `deployments/testnet.json` with all contract IDs.

## API Server

```bash
cd keeper
./api -config deployments/testnet.json -addr :8080 -db soroperps.db
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/health` | Health check |
| GET | `/api/v1/positions?trader=&status=open&limit=50` | List positions |
| GET | `/api/v1/positions/{id}` | Get position by ID |
| GET | `/api/v1/trades?trader=&limit=50` | Trade history |
| GET | `/api/v1/funding?limit=50` | Funding rate history |
| GET | `/api/v1/liquidations?limit=50` | Liquidation events |
| GET | `/api/v1/stats` | Protocol stats (TVL, OI, volume) |
| GET | `/api/v1/market/{asset}` | Per-asset market data |
| WS | `/ws` | Real-time event stream |

## Keeper Bot

```bash
cd keeper
./keeper -config deployments/testnet.json
```

Runs the liquidation scanner and funding rate trigger as background goroutines.

## Contract Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_leverage` | 50 | Maximum position leverage |
| `open_fee_bps` | 10 | Opening fee (0.1%) |
| `close_fee_bps` | 10 | Closing fee (0.1%) |
| `min_collateral` | 100_0000000 | Minimum collateral (100 USDC) |
| `max_utilization_bps` | 8000 | Max vault utilization (80%) |
| `maintenance_margin_rate` | 500 | Maintenance margin (5%) |
| `liquidation_penalty` | 250 | Liquidation penalty (2.5%) |
| `keeper_reward` | 100 | Keeper reward (1%) |
| `funding_interval` | 3600 | Funding rate interval (1 hour) |
| `max_funding_rate` | 100 | Max funding rate (1%) |
| `staleness_threshold` | 300 | Oracle staleness (5 minutes) |

## License

MIT
