use super::common::*;
use tipjar::TipJarError;

pub fn test_upgrade_versioning_and_state_preservation() {
    let ctx = TestContext::new();

    let sender = ctx.create_user();
    let creator = ctx.create_creator();

    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &500);

    assert_eq!(ctx.tipjar_client.get_version(), 0);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 500);

    let wasm_hash = ctx.env.deployer().upload_contract_wasm(tipjar::TipJarContract::wasm());
    ctx.tipjar_client.upgrade(&ctx.admin, &wasm_hash);

    assert_eq!(ctx.tipjar_client.get_version(), 1);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 500);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 500);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &250);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 750);
}

pub fn test_upgrade_authorization() {
    let ctx = TestContext::new();
    let user = ctx.create_user();

    let wasm_hash = ctx.env.deployer().upload_contract_wasm(tipjar::TipJarContract::wasm());
    let result = ctx.tipjar_client.try_upgrade(&user, &wasm_hash);

    assert_error_contains(result, TipJarError::UpgradeUnauthorized);
}

pub fn test_upgrade_rollback() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();

    ctx.mint_tokens(&sender, &ctx.token_1, 500);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &200);

    let wasm_hash = ctx.env.deployer().upload_contract_wasm(tipjar::TipJarContract::wasm());
    ctx.tipjar_client.upgrade(&ctx.admin, &wasm_hash);
    assert_eq!(ctx.tipjar_client.get_version(), 1);

    // Roll back by re-applying the same WASM hash. Version history should still increment.
    ctx.tipjar_client.upgrade(&ctx.admin, &wasm_hash);
    assert_eq!(ctx.tipjar_client.get_version(), 2);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 200);
}
