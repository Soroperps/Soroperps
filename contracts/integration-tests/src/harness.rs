use soroban_sdk::{testutils::Address as _, testutils::Ledger, token, Address, Env};

use perps_funding_rate::FundingRateContract;
use perps_funding_rate::FundingRateContractClient;
use perps_liquidation_engine::LiquidationEngineContract;
use perps_liquidation_engine::LiquidationEngineContractClient;
use perps_oracle_adapter::OracleAdapterContract;
use perps_oracle_adapter::OracleAdapterContractClient;
use perps_position_manager::PositionManagerContract;
use perps_position_manager::PositionManagerContractClient;
use perps_vault::VaultContract;
use perps_vault::VaultContractClient;

/// Full system harness: deploys and wires all 5 contracts together.
pub struct TestHarness<'a> {
    pub env: Env,
    pub admin: Address,
    pub usdc_address: Address,
    pub vault: VaultContractClient<'a>,
    pub oracle: OracleAdapterContractClient<'a>,
    pub pm: PositionManagerContractClient<'a>,
    pub le: LiquidationEngineContractClient<'a>,
    pub funding: FundingRateContractClient<'a>,
    #[allow(dead_code)]
    pub vault_address: Address,
}

impl<'a> TestHarness<'a> {
    pub fn setup() -> TestHarness<'static> {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let usdc_admin = Address::generate(&env);
        let usdc_contract = env.register_stellar_asset_contract_v2(usdc_admin.clone());
        let usdc_address = usdc_contract.address();

        // 1. Deploy vault
        let vault_address = env.register(VaultContract, ());
        let vault = VaultContractClient::new(&env, &vault_address);
        vault.initialize(&admin, &usdc_address, &8000_u32); // 80% max utilization

        // 2. Deploy oracle adapter
        let oracle_address = env.register(OracleAdapterContract, ());
        let oracle = OracleAdapterContractClient::new(&env, &oracle_address);
        oracle.initialize(&admin, &300_u64); // 300s staleness

        // 3. Deploy position manager
        let pm_address = env.register(PositionManagerContract, ());
        let pm = PositionManagerContractClient::new(&env, &pm_address);
        pm.initialize(
            &admin,
            &vault_address,
            &oracle_address,
            &usdc_address,
            &10_u32,          // 0.1% open fee
            &10_u32,          // 0.1% close fee
            &30_u32,          // max 30x leverage
            &10_0000000_i128, // min 10 USDC collateral
        );

        // 4. Deploy liquidation engine
        let le_address = env.register(LiquidationEngineContract, ());
        let le = LiquidationEngineContractClient::new(&env, &le_address);
        le.initialize(
            &admin,
            &pm_address,
            &oracle_address,
            &500_u32, // 5% maintenance margin
            &250_u32, // 2.5% liquidation penalty
            &50_u32,  // 0.5% keeper reward
        );

        // 5. Deploy funding rate
        let funding_address = env.register(FundingRateContract, ());
        let funding = FundingRateContractClient::new(&env, &funding_address);
        funding.initialize(
            &admin,
            &pm_address,
            &3600_u64, // 1 hour funding interval
            &10_u32,   // max 0.1% funding rate per interval
            &5_u32,    // 0.05% protocol spread
        );

        // Wire contracts together
        vault.set_position_manager(&admin, &pm_address);
        pm.set_liquidation_engine(&admin, &le_address);
        pm.set_funding_address(&admin, &funding_address);

        // Set initial timestamp
        env.ledger().with_mut(|li| {
            li.timestamp = 1000;
        });

        TestHarness {
            env,
            admin,
            usdc_address,
            vault,
            oracle,
            pm,
            le,
            funding,
            vault_address,
        }
    }

    /// Mint USDC to an address.
    pub fn mint_usdc(&self, to: &Address, amount: i128) {
        let admin_client = token::StellarAssetClient::new(&self.env, &self.usdc_address);
        admin_client.mint(to, &amount);
    }

    /// Create a funded trader.
    pub fn create_trader(&self, usdc_amount: i128) -> Address {
        let trader = Address::generate(&self.env);
        self.mint_usdc(&trader, usdc_amount);
        trader
    }

    /// Add LP liquidity. Returns the LP address.
    pub fn add_liquidity(&self, amount: i128) -> Address {
        let lp = Address::generate(&self.env);
        self.mint_usdc(&lp, amount);
        self.vault.deposit(&lp, &amount);
        lp
    }

    /// Set oracle price for an asset.
    pub fn set_price(&self, asset: u32, price: i128) {
        self.oracle.set_price(&self.admin, &asset, &price);
    }

    /// Get USDC balance of an address.
    pub fn usdc_balance(&self, account: &Address) -> i128 {
        let token_client = token::TokenClient::new(&self.env, &self.usdc_address);
        token_client.balance(account)
    }

    /// Advance ledger timestamp.
    pub fn advance_time(&self, seconds: u64) {
        self.env.ledger().with_mut(|li| {
            li.timestamp += seconds;
        });
    }
}
