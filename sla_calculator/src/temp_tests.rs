// ============================================================
// Batch Size Boundary Enforcement Tests
// ============================================================

#[test]
#[should_panic(expected = "InvalidThreshold")]
fn test_empty_batch_rejected() {
    let (env, client, actors) = setup();
    // Create empty batch
    let empty_requests = Vec::<BatchRequest>::new(&env);
    client.batch_calculate(&actors.operator, &empty_requests);
}

#[test]
#[should_panic(expected = "InvalidThreshold")]
fn test_oversized_batch_rejected() {
    let (env, client, actors) = setup();
    // Create batch with 51 items (exceeds limit of 50)
    let mut requests = Vec::<BatchRequest>::new(&env);
    for i in 0..51 {
        requests.push_back(BatchRequest {
            outage_id: symbol(&env, &format!("OUTAGE_{}", i)),
            severity: symbol_short!("high"),
            mttr_minutes: 10,
        });
    }
    client.batch_calculate(&actors.operator, &requests);
}

#[test]
fn test_batch_at_max_limit_accepted() {
    let (env, client, actors) = setup();
    // Create batch with exactly 50 items (max limit)
    let mut requests = Vec::<BatchRequest>::new(&env);
    for i in 0..50 {
        requests.push_back(BatchRequest {
            outage_id: symbol(&env, &format!("OUTAGE_{}", i)),
            severity: symbol_short!("high"),
            mttr_minutes: 10,
        });
    }
    let result = client.try_batch_calculate(&actors.operator, &requests);
    assert!(result.is_ok());
}

#[test]
fn test_small_batch_accepted() {
    let (env, client, actors) = setup();
    // Create small valid batch
    let mut requests = Vec::<BatchRequest>::new(&env);
    requests.push_back(BatchRequest {
        outage_id: symbol_short!("OUT01"),
        severity: symbol_short!("high"),
        mttr_minutes: 10,
    });
    requests.push_back(BatchRequest {
        outage_id: symbol_short!("OUT02"),
        severity: symbol_short!("medium"),
        mttr_minutes: 20,
    });
    let result = client.try_batch_calculate(&actors.operator, &requests);
    assert!(result.is_ok());
}

#[test]
#[should_panic(expected = "DuplicateOutageInput")]
fn test_batch_with_duplicate_outage_ids_rejected() {
    let (env, client, actors) = setup();
    // Create batch with duplicate outage IDs (should be caught by validate_batch)
    let mut requests = Vec::<BatchRequest>::new(&env);
    requests.push_back(BatchRequest {
        outage_id: symbol_short!("OUT01"),
        severity: symbol_short!("high"),
        mttr_minutes: 10,
    });
    requests.push_back(BatchRequest {
        outage_id: symbol_short!("OUT01"), // Same ID as above
        severity: symbol_short!("medium"),
        mttr_minutes: 20,
    });
    client.batch_calculate(&actors.operator, &requests);
}