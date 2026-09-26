use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};
use soroban_sdk::testutils::Address as _;

use crate::{SLACalculatorContract, SLAResult};

#[test]
fn test_get_batch_downtime_aggregates_history_and_compliance() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = crate::SLACalculatorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let zero_hash = soroban_sdk::BytesN::from_array(&env, &[0; 32]);
    let mut history = Vec::<SLAResult>::new(&env);

    history.push_back(SLAResult {
        outage_id: symbol_short!("SITE_A"),
        status: symbol_short!("met"),
        mttr_minutes: 10,
        threshold_minutes: 15,
        amount: 750,
        payment_type: symbol_short!("rew"),
        rating: symbol_short!("good"),
        config_version_hash: zero_hash.clone(),
        recorded_at: 1,
    });
    history.push_back(SLAResult {
        outage_id: symbol_short!("SITE_A"),
        status: symbol_short!("viol"),
        mttr_minutes: 20,
        threshold_minutes: 15,
        amount: -500,
        payment_type: symbol_short!("pen"),
        rating: symbol_short!("poor"),
        config_version_hash: zero_hash.clone(),
        recorded_at: 2,
    });
    history.push_back(SLAResult {
        outage_id: symbol_short!("SITE_B"),
        status: symbol_short!("met"),
        mttr_minutes: 5,
        threshold_minutes: 15,
        amount: 750,
        payment_type: symbol_short!("rew"),
        rating: symbol_short!("top"),
        config_version_hash: zero_hash,
        recorded_at: 3,
    });

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&crate::HISTORY_KEY, &history);
    });

    let mut site_ids = Vec::<Symbol>::new(&env);
    site_ids.push_back(symbol_short!("SITE_A"));
    site_ids.push_back(symbol_short!("SITE_B"));
    site_ids.push_back(symbol_short!("SITE_C"));

    let results = client.get_batch_downtime(&site_ids);
    assert_eq!(results.len(), 3);

    let site_a = results.get(0).unwrap();
    assert_eq!(site_a.site_id, symbol_short!("SITE_A"));
    assert_eq!(site_a.total_downtime, 30);
    assert_eq!(site_a.compliance_status, symbol_short!("noncomp"));

    let site_b = results.get(1).unwrap();
    assert_eq!(site_b.site_id, symbol_short!("SITE_B"));
    assert_eq!(site_b.total_downtime, 5);
    assert_eq!(site_b.compliance_status, symbol_short!("compliant"));

    let site_c = results.get(2).unwrap();
    assert_eq!(site_c.total_downtime, 0);
    assert_eq!(site_c.compliance_status, symbol_short!("unknown"));
}

#[test]
fn test_get_batch_downtime_rejects_more_than_50_site_ids() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = crate::SLACalculatorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let mut site_ids = Vec::<Symbol>::new(&env);
    for i in 0..51 {
        site_ids.push_back(Symbol::new(&env, &format!("SITE{}", i)));
    }

    let result = client.try_get_batch_downtime(&site_ids);
    assert_eq!(result, Err(Ok(crate::SLAError::ThresholdOutOfBounds)));
}

#[test]
fn test_get_batch_downtime_50_item_performance() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = crate::SLACalculatorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let zero_hash = soroban_sdk::BytesN::from_array(&env, &[0; 32]);
    let mut history = Vec::<SLAResult>::new(&env);
    let mut site_ids = Vec::<Symbol>::new(&env);

    for i in 0..50 {
        let site_id = Symbol::new(&env, &format!("S{}", i));
        site_ids.push_back(site_id.clone());
        history.push_back(SLAResult {
            outage_id: site_id,
            status: symbol_short!("met"),
            mttr_minutes: 10,
            threshold_minutes: 15,
            amount: 750,
            payment_type: symbol_short!("rew"),
            rating: symbol_short!("good"),
            config_version_hash: zero_hash.clone(),
            recorded_at: i as u64,
        });
    }

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&crate::HISTORY_KEY, &history);
    });

    env.budget().reset_tracker();
    let results = client.get_batch_downtime(&site_ids);
    assert_eq!(results.len(), 50);

    let cpu = env.budget().cpu_instruction_cost();
    assert!(
        cpu < 5_000_000,
        "50-site batch getter consumed {} CPU instructions",
        cpu
    );
}
