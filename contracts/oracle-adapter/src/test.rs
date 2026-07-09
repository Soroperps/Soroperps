use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, Env};

use crate::OracleAdapterContract;
use crate::OracleAdapterContractClient;

fn setup() -> (Env, OracleAdapterContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(OracleAdapterContract, ());
    let client = OracleAdapterContractClient::new(&env, &contract_id);

    // staleness threshold = 300 seconds (5 minutes)
    client.initialize(&admin, &300_u64);

    (env, client, admin)
}

#[test]
fn test_initialize() {
    let (_env, _client, _admin) = setup();
    // No panic = success
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_initialize_twice_fails() {
    let (_env, client, admin) = setup();
    client.initialize(&admin, &300_u64);
}

#[test]
fn test_set_and_get_price() {
    let (env, client, admin) = setup();

    // Set ledger timestamp
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    // Asset 0 = XLM at $0.15 (with 7 decimals = 1_500_000)
    client.set_price(&admin, &0_u32, &1_500_000_i128);

    let cached = client.get_price(&0_u32);
    assert_eq!(cached.price, 1_500_000);
    assert_eq!(cached.timestamp, 1000);
}

#[test]
fn test_get_price_value() {
    let (env, client, admin) = setup();

    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    client.set_price(&admin, &1_u32, &500_000_0000000_i128); // BTC at $50,000

    let price = client.get_price_value(&1_u32);
    assert_eq!(price, 500_000_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_get_price_stale() {
    let (env, client, admin) = setup();

    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    client.set_price(&admin, &0_u32, &1_500_000_i128);

    // Advance time past staleness threshold (300s)
    env.ledger().with_mut(|li| {
        li.timestamp = 1400;
    });

    // Should fail — price is 400s old, threshold is 300s
    client.get_price(&0_u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_get_price_missing() {
    let (_env, client, _admin) = setup();

    // No price set for asset 5
    client.get_price(&5_u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_set_price_unauthorized() {
    let (env, client, _admin) = setup();

    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    let rando = Address::generate(&env);
    client.set_price(&rando, &0_u32, &1_500_000_i128);
}

#[test]
fn test_update_staleness_threshold() {
    let (env, client, admin) = setup();

    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    client.set_price(&admin, &0_u32, &1_500_000_i128);

    // Advance 500s — stale with 300s threshold
    env.ledger().with_mut(|li| {
        li.timestamp = 1500;
    });

    // Update threshold to 600s
    client.set_staleness_threshold(&admin, &600_u64);

    // Now price is fresh (500s < 600s threshold)
    let cached = client.get_price(&0_u32);
    assert_eq!(cached.price, 1_500_000);
}
