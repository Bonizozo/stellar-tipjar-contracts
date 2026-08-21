use tipjar_simulator::{state_manager, Simulator};

#[test]
fn test_tip_and_withdraw_flow() {
    let sim = Simulator::new(false);
    let sender = sim.new_address();
    let creator = sim.new_address();
    sim.mint(&sender, 500);

    let r = sim.simulate_tip(&sender, &creator, 300);
    assert!(r.ok, "tip should succeed");
    assert_eq!(sim.balance(&creator), 300);
    assert_eq!(sim.total_tips(&creator), 300);

    let r = sim.simulate_withdraw(&creator);
    assert!(r.ok, "withdraw should succeed");
    assert_eq!(sim.balance(&creator), 0);
    assert_eq!(sim.total_tips(&creator), 300); // historical total unchanged
}

#[test]
fn test_invalid_tip_rejected() {
    let sim = Simulator::new(false);
    let sender = sim.new_address();
    let creator = sim.new_address();
    // No mint — sender has no balance.
    let r = sim.simulate_tip(&sender, &creator, 100);
    assert!(!r.ok);
    assert!(r.error.is_some());
}

#[test]
fn test_cpu_instructions_reported() {
    let sim = Simulator::new(false);
    let sender = sim.new_address();
    let creator = sim.new_address();
    sim.mint(&sender, 1000);

    let r = sim.simulate_tip(&sender, &creator, 100);
    assert!(r.ok);
    assert!(
        r.cpu_instructions > 0,
        "cpu instructions should be non-zero"
    );
}

#[test]
#[ignore = "snapshot restore cannot reuse Address object references across Soroban Env instances"]
fn test_snapshot_and_restore() {
    let sim = Simulator::new(false);
    let sender = sim.new_address();
    let creator = sim.new_address();
    sim.mint(&sender, 1000);
    sim.simulate_tip(&sender, &creator, 400);

    let snap = sim.snapshot();

    let mut sim2 = Simulator::new(false);
    sim2.restore_snapshot(snap);

    assert_eq!(sim2.balance(&creator), 400);
}

#[test]
#[ignore = "state file restore cannot reuse Address object references across Soroban Env instances"]
fn test_state_file_roundtrip() {
    use std::collections::HashMap;
    let path = std::env::temp_dir().join("tipjar_sim_test_state.json");

    let sim = Simulator::new(false);
    let sender = sim.new_address();
    let creator = sim.new_address();
    sim.mint(&sender, 200);
    sim.simulate_tip(&sender, &creator, 200);

    let snap = sim.snapshot();
    let mut labels = HashMap::new();
    labels.insert(format!("{creator:?}"), "creator".into());
    state_manager::save(&path, snap, labels).unwrap();

    let (loaded_snap, loaded_labels) = state_manager::load(&path).unwrap();
    assert!(loaded_labels.values().any(|v| v == "creator"));

    let mut sim2 = Simulator::new(false);
    sim2.restore_snapshot(loaded_snap);
    assert_eq!(sim2.balance(&creator), 200);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_time_advance() {
    let sim = Simulator::new(false);
    let t0 = sim.env.ledger().timestamp();
    sim.advance_time(3600);
    assert_eq!(sim.env.ledger().timestamp() - t0, 3600);
}

#[test]
fn test_multiple_creators() {
    let sim = Simulator::new(false);
    let sender = sim.new_address();
    let c1 = sim.new_address();
    let c2 = sim.new_address();
    sim.mint(&sender, 1000);

    sim.simulate_tip(&sender, &c1, 300);
    sim.simulate_tip(&sender, &c2, 500);

    assert_eq!(sim.balance(&c1), 300);
    assert_eq!(sim.balance(&c2), 500);
}
