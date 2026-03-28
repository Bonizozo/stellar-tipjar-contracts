use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    token, Address, Env, String as SorobanString, Map, Vec as SorobanVec,
};
use tipjar::{
    TipJarContract, TipJarContractClient, TipJarError, BatchTip, LockedTip, 
    MatchingProgram, Role, TimePeriod, TipWithMessage, LeaderboardEntry, TipHistoryQuery
};

mod common;
use common::*;

mod core_functionality;
mod advanced_features;
mod edge_cases;
mod failure_scenarios;
mod gas_analysis;
mod property_tests;
mod cross_contract;

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Comprehensive integration test suite entry point
    #[test]
    fn run_comprehensive_integration_tests() {
        // Core functionality tests
        core_functionality::test_complete_tip_workflows();
        core_functionality::test_role_based_operations();
        core_functionality::test_token_management_workflows();
        core_functionality::test_pause_unpause_workflows();
        core_functionality::test_upgrade_workflows();

        // Advanced features tests
        advanced_features::test_batch_operations();
        advanced_features::test_locked_tips_workflows();
        advanced_features::test_matching_programs();
        advanced_features::test_leaderboard_functionality();
        advanced_features::test_cross_contract_integrations();

        // Edge cases tests
        edge_cases::test_boundary_conditions();
        edge_cases::test_concurrent_operations();
        edge_cases::test_malformed_inputs();

        // Failure scenarios tests
        failure_scenarios::test_insufficient_balance_scenarios();
        failure_scenarios::test_unauthorized_access();
        failure_scenarios::test_invalid_token_operations();
        failure_scenarios::test_time_based_failures();

        // Gas analysis tests
        gas_analysis::test_basic_operation_costs();
        gas_analysis::test_batch_operation_efficiency();
        gas_analysis::test_complex_operation_costs();

        // Property-based tests
        property_tests::test_balance_conservation_properties();
        property_tests::test_authorization_properties();
        property_tests::test_leaderboard_consistency_properties();

        // Cross-contract integration tests
        cross_contract::test_dex_integration();
        cross_contract::test_nft_integration();
        cross_contract::test_external_contract_failures();
    }
}