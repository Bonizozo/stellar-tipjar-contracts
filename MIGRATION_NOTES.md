# Multi-Token Migration Changes

This document outlines the changes made to implement multi-token support with lazy v1→v2 migration as specified in issue #357.

## Overview

The tip jar contract has been upgraded from a single-token system to support multiple tokens with a bounded allowlist. The migration is designed to be backward-compatible and lossless.

## Key Changes

### 1. Storage Redesign

**v1 Storage Keys:**
- `DataKey::Token` - Single token address
- `DataKey::CreatorBalance(Address)` - Creator's balance
- `DataKey::CreatorTotal(Address)` - Creator's historical total

**v2 Storage Keys:**
- `DataKey::DataVersion` - Version tracking for migration
- `DataKey::AllowedTokens` - Vector of allowed token addresses (max 50)
- `DataKey::Balance(Address, Address)` - Creator balance per token
- `DataKey::Total(Address, Address)` - Creator total per token

### 2. New Contract Methods

**Multi-token API:**
- `tip(sender, creator, token, amount)` - Tip with specific token
- `withdraw(caller, creator, token, to, amount?)` - Withdraw specific token
- `get_balance(creator, token)` - Get balance for creator/token pair
- `get_total_tips(creator, token)` - Get total for creator/token pair
- `get_tokens()` - Get allowed tokens list

**Admin Methods:**
- `add_token(admin, token)` - Add token to allowlist
- `remove_token(admin, token)` - Remove token from allowlist

**Legacy Compatibility:**
- `tip_legacy(sender, creator, amount)` - Uses first token in allowlist
- `withdraw_legacy(caller, creator, to, amount?)` - Uses first token
- `get_total_tips_legacy(creator)` - Uses first token
- `token_address()` - Returns legacy token or first allowed token

### 3. Lazy Migration

The migration from v1 to v2 happens automatically and lazily:

1. **On First Access**: When `get_balance`, `get_total_tips`, or `tip` is called
2. **Data Preservation**: v1 balances and totals are copied to v2 format
3. **Cleanup**: v1 data is removed after successful migration
4. **Allowlist Initialization**: Legacy token is added to allowlist
5. **Version Upgrade**: DataVersion is set to 2

### 4. Event Schema Changes

**v1 Events:**
- `Tip { creator, sender, amount }`
- `Withdraw { creator, amount, to }`

**v2 Events:**
- `Tip { creator, token, sender, amount }`
- `Withdraw { creator, token, amount, to }`

### 5. Error Handling

New error types added:
- `TokenNotAllowed` - Tipping with non-allowlisted token
- `TokenAlreadyExists` - Adding duplicate token
- `MaxTokensReached` - Exceeding 50 token limit
- `Unauthorized` - Admin-only operations

## Migration Guarantees

1. **Lossless**: All v1 balances and totals are preserved
2. **Automatic**: No manual intervention required
3. **Lazy**: Migration happens on first access per creator/token
4. **Backward Compatible**: Legacy API methods still work
5. **Atomic**: Each creator's data migrates atomically

## Testing

Comprehensive tests added in `test_multitoken.rs`:

- Multi-token balance isolation
- Token allowlist management
- v1→v2 migration correctness
- Event schema validation
- Error condition handling
- Overflow protection per token
- Legacy API compatibility

## Indexer Changes

Updated event parsing in `indexer/src/event_parser.rs`:
- `parse_tip()` now expects `[token, sender, amount]`
- `parse_withdraw()` now expects `[token, amount, to]`

## Usage Examples

```rust
// v2 Multi-token API
client.add_token(&admin, &usdc_token);
client.tip(&sender, &creator, &usdc_token, &100);
client.withdraw(&creator, &creator, &usdc_token, &creator, &Some(50));

// Legacy API (still works)
client.tip_legacy(&sender, &creator, &100); // Uses first allowed token
client.withdraw_legacy(&creator, &creator, &creator, &None);
```

## Deployment Notes

1. **Backward Compatibility**: Existing v1 contracts continue to work
2. **No Data Loss**: All existing balances are preserved during migration
3. **Gradual Migration**: Users migrate individually on first interaction
4. **Admin Setup**: New tokens must be added to allowlist by admin
5. **Client Updates**: Frontend should be updated to use new event schema

## Testing Results

All tests pass including:
- ✅ Multi-token balance isolation
- ✅ Lazy v1→v2 migration 
- ✅ Token allowlist enforcement
- ✅ Event schema with token addresses
- ✅ Error handling for edge cases
- ✅ Backward compatibility
- ✅ Overflow protection per token bucket

The implementation fully satisfies the requirements specified in issue #357.
