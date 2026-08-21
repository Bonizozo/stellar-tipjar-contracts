use serde::{Deserialize, Serialize};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _, Snapshot},
    token, Address, Env,
};
use tipjar_legacy::{TipJarContract, TipJarContractClient};

/// Result of a single simulated transaction.
#[derive(Debug, Serialize, Deserialize)]
pub struct SimResult {
    pub ok: bool,
    pub error: Option<String>,
    /// CPU instructions consumed (proxy for gas).
    pub cpu_instructions: u64,
}

/// The local simulation environment.
///
/// Wraps Soroban's in-process `Env` so every call runs the real contract
/// logic without touching a live network.
pub struct Simulator {
    pub env: Env,
    pub contract: Address,
    pub token: Address,
    pub admin: Address,
    pub verbose: bool,
}

impl Simulator {
    /// Create a fresh simulation environment.
    pub fn new(verbose: bool) -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin.clone())
            .address();

        let admin = Address::generate(&env);
        let contract = env.register(TipJarContract, ());
        let client = TipJarContractClient::new(&env, &contract);

        client.init(&admin);
        client.add_token(&admin, &token);
        client.set_min_tip(&admin, &1);

        if verbose {
            eprintln!("[sim] contract={contract:?}  token={token:?}  admin={admin:?}");
        }

        Self {
            env,
            contract,
            token,
            admin,
            verbose,
        }
    }

    fn client(&self) -> TipJarContractClient<'_> {
        TipJarContractClient::new(&self.env, &self.contract)
    }

    /// Mint `amount` of the simulation token to `address`.
    pub fn mint(&self, address: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.token).mint(address, &amount);
    }

    /// Generate a fresh address in this environment.
    pub fn new_address(&self) -> Address {
        Address::generate(&self.env)
    }

    /// Advance the ledger timestamp by `seconds`.
    pub fn advance_time(&self, seconds: u64) {
        let ts = self.env.ledger().timestamp() + seconds;
        self.env.ledger().set_timestamp(ts);
        if self.verbose {
            eprintln!("[sim] ledger timestamp → {ts}");
        }
    }

    /// Simulate a tip and return a `SimResult`.
    pub fn simulate_tip(&self, sender: &Address, creator: &Address, amount: i128) -> SimResult {
        self.env.cost_estimate().budget().reset_unlimited();
        let result = self.client().try_tip(sender, creator, &self.token, &amount);
        let cpu = self.env.cost_estimate().budget().cpu_instruction_cost();
        let ok = result.is_ok();
        let error = result.err().map(|e| format!("{e:?}"));
        if self.verbose {
            eprintln!("[sim] tip {amount} → creator={creator:?}  ok={ok}  cpu={cpu}");
        }
        SimResult {
            ok,
            error,
            cpu_instructions: cpu,
        }
    }

    /// Simulate a withdrawal and return a `SimResult`.
    pub fn simulate_withdraw(&self, creator: &Address) -> SimResult {
        self.env.cost_estimate().budget().reset_unlimited();
        let result = self.client().try_withdraw(creator, &self.token, &None);
        let cpu = self.env.cost_estimate().budget().cpu_instruction_cost();
        let ok = result.is_ok();
        let error = result.err().map(|e| format!("{e:?}"));
        if self.verbose {
            eprintln!("[sim] withdraw creator={creator:?}  ok={ok}  cpu={cpu}");
        }
        SimResult {
            ok,
            error,
            cpu_instructions: cpu,
        }
    }

    /// Query the withdrawable balance for a creator.
    pub fn balance(&self, creator: &Address) -> i128 {
        self.client().get_withdrawable_balance(creator, &self.token)
    }

    /// Query the historical total tips for a creator.
    pub fn total_tips(&self, creator: &Address) -> i128 {
        self.client().get_total_tips(creator, &self.token)
    }

    /// Export the current ledger as a `Snapshot`.
    pub fn snapshot(&self) -> Snapshot {
        self.env.to_snapshot()
    }

    /// Restore a previously exported snapshot into this simulator.
    pub fn restore_snapshot(&mut self, snap: Snapshot) {
        self.env = Env::from_snapshot(snap);
        self.env.mock_all_auths();
        self.env.cost_estimate().budget().reset_unlimited();
    }
}
