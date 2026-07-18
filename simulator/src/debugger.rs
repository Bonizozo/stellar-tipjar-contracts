use soroban_sdk::testutils::Events as _;

use crate::simulator::Simulator;

/// Print all events emitted since the last call to `env.events().all()`.
pub fn dump_events(sim: &Simulator) {
    let events = sim.env.events().all();
    if events.is_empty() {
        println!("[dbg] no events");
        return;
    }
    for (i, event) in events.iter().enumerate() {
        println!("[dbg] event[{i}]: {event:?}");
    }
}

/// Print a labelled balance snapshot for a set of addresses.
pub fn dump_balances(sim: &Simulator, accounts: &[(&str, &soroban_sdk::Address)]) {
    for (label, addr) in accounts {
        let bal = sim.balance(addr);
        let total = sim.total_tips(addr);
        println!("[dbg] {label}: withdrawable={bal}  total_tips={total}");
    }
}

/// Print current ledger timestamp and sequence.
pub fn dump_ledger(sim: &Simulator) {
    use soroban_sdk::testutils::Ledger as _;
    let ts = sim.env.ledger().timestamp();
    let seq = sim.env.ledger().sequence();
    println!("[dbg] ledger: sequence={seq}  timestamp={ts}");
}
