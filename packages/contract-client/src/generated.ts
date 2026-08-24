import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}


export const networks = {
  testnet: {
    networkPassphrase: "Test SDF Network ; September 2015",
    contractId: "CCS6SYADTODLLKHTXKMBFJXPK4FY3BMLZ2624XEFDSOXY22JQPIIEIEA",
  }
} as const






export const Errors = {
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"InvalidAmount"},
  4: {message:"NothingToWithdraw"},
  5: {message:"InvalidOperator"},
  6: {message:"OperatorExpired"},
  7: {message:"InsufficientAllowance"},
  8: {message:"PendingPayoutChangeActive"},
  9: {message:"NoPendingPayoutChange"},
  10: {message:"InvalidTarget"},
  11: {message:"Unauthorized"},
  12: {message:"NoPendingAdmin"},
  13: {message:"InvalidTimelock"},
  14: {message:"UpgradeAlreadyPending"},
  15: {message:"NoPendingUpgrade"},
  16: {message:"TimelockNotElapsed"},
  17: {message:"TipsPaused"},
  18: {message:"WithdrawalsPaused"},
  19: {message:"InvalidDuration"},
  20: {message:"InvalidFee"},
  21: {message:"FeeOverflow"},
  22: {message:"NotFeeCollector"},
  23: {message:"TokenNotAllowed"},
  24: {message:"TokenAlreadyExists"},
  25: {message:"MaxTokensReached"}
}


export type DataKey = {tag: "Token", values: void} | {tag: "CreatorBalance", values: readonly [string]} | {tag: "CreatorTotal", values: readonly [string]} | {tag: "AllowedTokens", values: void} | {tag: "Balance", values: readonly [string, string]} | {tag: "Total", values: readonly [string, string]} | {tag: "PayoutAddress", values: readonly [string]} | {tag: "PendingPayoutChange", values: readonly [string]} | {tag: "Operator", values: readonly [string, string]} | {tag: "Admin", values: void} | {tag: "PendingAdmin", values: void} | {tag: "UpgradeTimelockLedgers", values: void} | {tag: "PendingUpgrade", values: void} | {tag: "DataVersion", values: void} | {tag: "Guardian", values: void} | {tag: "Pause", values: void} | {tag: "GuardianPauseDuration", values: void} | {tag: "FeeBps", values: void} | {tag: "FeeCollector", values: void} | {tag: "FeeBalance", values: void} | {tag: "FeeBalanceToken", values: readonly [string]};





/**
 * Circuit-breaker state, stored as a single instance-storage entry.
 * 
 * `admin_flags` and `guardian_flags` are independent bitmasks over
 * `PAUSE_FLAG_*` rather than separate booleans, so tips/withdrawals pause
 * independently. They're kept in two buckets (rather than one shared
 * bitmask) so a guardian's temporary pause can never silently overwrite, or
 * be silently promoted into, an admin's deliberate persistent pause:
 * - `admin_flags` bits are set only by the admin and never auto-expire;
 * only an admin `unpause_*` call clears them.
 * - `guardian_flags` bits are set only by the guardian and auto-expire at
 * `guardian_expiry` (a single shared ledger checkpoint) unless the admin
 * confirms them first by calling the matching `pause_*`, which promotes
 * them into `admin_flags` and clears them here.
 */
export interface PauseState {
  admin_flags: u32;
  guardian_expiry: u32;
  guardian_flags: u32;
}
















export interface Client {
  /**
   * Construct and simulate a tip transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Escrows `amount` of the configured token from `sender` for `creator`,
   * less the protocol fee (if one is configured). The creator's balance is
   * credited with `amount - fee`; the fee itself accrues to
   * `FeeBalance(token)` for later withdrawal by the fee collector.
   * `fee + net == amount` holds for every input.
   */
  tip: ({sender, creator, token, amount}: {sender: string, creator: string, token: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a init transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time configuration: the token this jar accepts (seeded as the
   * first entry of the multi-token allowlist), the admin, and the ledger
   * delay `execute_upgrade` must wait out after a `propose_upgrade`.
   * Errors if called twice.
   */
  init: ({token, admin, upgrade_timelock_ledgers}: {token: string, admin: string, upgrade_timelock_ledgers: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a migrate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Admin-only, idempotent. Advances `DataKey::DataVersion` towards this
   * build's `DATA_VERSION`, applying any storage transformation the new
   * WASM requires. A no-op (no panic, no event) if the stored version
   * already meets or exceeds `DATA_VERSION` — safe to call more than once,
   * including before the first upgrade or after a repeated invocation.
   */
  migrate: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sets the protocol fee rate and its collector. Admin-only; `bps` is
   * hard-capped at `MAX_FEE_BPS`. Setting `bps` to 0 disables fees.
   */
  set_fee: ({admin, bps, collector}: {admin: string, bps: u32, collector: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pays out a creator's withdrawable balance for a specific token.
   */
  withdraw: ({caller, creator, token, to, amount}: {caller: string, creator: string, token: string, to: string, amount: Option<i128>}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a add_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Adds a token to the allowlist. Admin-only operation.
   */
  add_token: ({admin, token}: {admin: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Current admin address.
   */
  get_admin: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a pause_all transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pauses both tips and withdrawals in one call. See `pause_tips`.
   */
  pause_all: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_tokens transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the list of allowed tokens.
   */
  get_tokens: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a pause_tips transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pauses `tip`. Callable by admin (persists until explicitly unpaused)
   * or guardian (auto-expires; call again as admin to confirm/persist it).
   */
  pause_tips: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a tip_legacy transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  tip_legacy: ({sender, creator, amount}: {sender: string, creator: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Gets the withdrawable balance for a creator and specific token.
   */
  get_balance: ({creator, token}: {creator: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_fee_bps transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_fee_bps: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a pause_flags transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * The currently effective pause bitmask (accounting for guardian auto-expiry).
   */
  pause_flags: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a preview_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Computes `(fee, net)` for `amount` at `bps` without touching storage.
   * Exposed read-only so off-chain callers (SDKs, indexers, tests) can
   * preview the exact split the contract will apply.
   */
  preview_fee: ({amount, bps}: {amount: i128, bps: u32}, options?: MethodOptions) => Promise<AssembledTransaction<readonly [i128, i128]>>

  /**
   * Construct and simulate a unpause_all transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Unpauses both tips and withdrawals in one call. Admin only.
   */
  unpause_all: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a accept_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Completes a two-step admin transfer. Must be called by the address
   * named in the pending proposal.
   */
  accept_admin: ({new_admin}: {new_admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_guardian transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_guardian: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a remove_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Removes a token from the allowlist. Admin-only operation.
   * Existing balances remain withdrawable.
   */
  remove_token: ({admin, token}: {admin: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_guardian transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Appoints (or replaces) the guardian. Admin only.
   */
  set_guardian: ({admin, guardian}: {admin: string, guardian: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a unpause_tips transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Unpauses `tip`. Admin only — guardians can pause but never unpause.
   */
  unpause_tips: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a propose_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Proposes `new_admin` as the next admin. Takes effect only once
   * `new_admin` calls `accept_admin` — a single-step transfer to a typo'd
   * or unreachable address can never permanently lock out administration.
   */
  propose_admin: ({admin, new_admin}: {admin: string, new_admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a withdraw_fees transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pays out the fee collector's full or partial share of
   * `FeeBalance(token)`. Only moves units of `token`. Mirrors `withdraw`'s
   * pattern, including TTL extension.
   */
  withdraw_fees: ({caller, token, amount}: {caller: string, token: string, amount: Option<i128>}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Admin-only. Aborts a pending upgrade proposal without waiting out the
   * timelock.
   */
  cancel_upgrade: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_total_tips transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Historical total ever tipped to `creator` for a specific `token`. Zero if never tipped.
   */
  get_total_tips: ({creator, token}: {creator: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a execute_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Swaps this contract's WASM to the proposed hash once its timelock has
   * elapsed. Permissionless by design — the admin already authorized the
   * upgrade at `propose_upgrade`, and its unlock ledger is public
   * on-chain state, so no caller identity check adds meaningful security
   * here. Storage is preserved by the host across the swap; call the new
   * WASM's `migrate()` afterwards to apply any storage-layout changes.
   */
  execute_upgrade: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_fee_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_fee_balance: ({token}: {token: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a propose_upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Admin-only. Records `new_wasm_hash` as a pending upgrade, unlocked
   * after the ledger delay configured at `init`. Only one proposal may be
   * pending at a time — cancel the existing one first to replace it.
   */
  propose_upgrade: ({admin, new_wasm_hash}: {admin: string, new_wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a revoke_operator transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  revoke_operator: ({creator, operator}: {creator: string, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a withdraw_legacy transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  withdraw_legacy: ({caller, creator, to, amount}: {caller: string, creator: string, to: string, amount: Option<i128>}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_data_version transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Current storage schema version.
   */
  get_data_version: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a get_fee_collector transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_fee_collector: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a get_pending_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Address currently proposed as the next admin, if any.
   */
  get_pending_admin: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a is_feature_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * True if every bit in `flag` is currently paused (accounting for
   * guardian auto-expiry).
   */
  is_feature_paused: ({flag}: {flag: u32}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a pause_withdrawals transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pauses `withdraw` and the withdrawal-mechanics entrypoints. See `pause_tips`.
   */
  pause_withdrawals: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a authorize_operator transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  authorize_operator: ({creator, operator, allowance, expiry_ledger}: {creator: string, operator: string, allowance: i128, expiry_ledger: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_payout_address transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_payout_address: ({creator, payout}: {creator: string, payout: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a unpause_withdrawals transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Unpauses `withdraw` and the withdrawal-mechanics entrypoints. Admin only.
   */
  unpause_withdrawals: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_admin_transfer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Abandons a pending admin transfer, leaving the current admin in
   * place. Admin-only.
   */
  cancel_admin_transfer: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a cancel_payout_address transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_payout_address: ({creator}: {creator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_total_tips_legacy transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_total_tips_legacy: ({creator}: {creator: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a set_guardian_pause_duration transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Configures how many ledgers a guardian-initiated pause lasts before
   * auto-expiring. Admin only.
   */
  set_guardian_pause_duration: ({admin, ledgers}: {admin: string, ledgers: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a guardian_pause_expiry_ledger transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Ledger sequence at which the current guardian-originated pause bits
   * expire. 0 if no guardian pause is active.
   */
  guardian_pause_expiry_ledger: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABQAAADpUb3BpY3MgYCgidGlwIiwgY3JlYXRvcilgLCBkYXRhIGAodG9rZW4sIHNlbmRlciwgYW1vdW50KWAuAAAAAAAAAAAAA1RpcAAAAAABAAAAA3RpcAAAAAAEAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAAAAAAAAZzZW5kZXIAAAAAABMAAAAAAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAQ==",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAAGQAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAMAAAAAAAAAEU5vdGhpbmdUb1dpdGhkcmF3AAAAAAAABAAAAAAAAAAPSW52YWxpZE9wZXJhdG9yAAAAAAUAAAAAAAAAD09wZXJhdG9yRXhwaXJlZAAAAAAGAAAAAAAAABVJbnN1ZmZpY2llbnRBbGxvd2FuY2UAAAAAAAAHAAAAAAAAABlQZW5kaW5nUGF5b3V0Q2hhbmdlQWN0aXZlAAAAAAAACAAAAAAAAAAVTm9QZW5kaW5nUGF5b3V0Q2hhbmdlAAAAAAAACQAAAAAAAAANSW52YWxpZFRhcmdldAAAAAAAAAoAAAAAAAAADFVuYXV0aG9yaXplZAAAAAsAAAAAAAAADk5vUGVuZGluZ0FkbWluAAAAAAAMAAAAAAAAAA9JbnZhbGlkVGltZWxvY2sAAAAADQAAAAAAAAAVVXBncmFkZUFscmVhZHlQZW5kaW5nAAAAAAAADgAAAAAAAAAQTm9QZW5kaW5nVXBncmFkZQAAAA8AAAAAAAAAElRpbWVsb2NrTm90RWxhcHNlZAAAAAAAEAAAAAAAAAAKVGlwc1BhdXNlZAAAAAAAEQAAAAAAAAARV2l0aGRyYXdhbHNQYXVzZWQAAAAAAAASAAAAAAAAAA9JbnZhbGlkRHVyYXRpb24AAAAAEwAAAAAAAAAKSW52YWxpZEZlZQAAAAAAFAAAAAAAAAALRmVlT3ZlcmZsb3cAAAAAFQAAAAAAAAAPTm90RmVlQ29sbGVjdG9yAAAAABYAAAAAAAAAD1Rva2VuTm90QWxsb3dlZAAAAAAXAAAAAAAAABJUb2tlbkFscmVhZHlFeGlzdHMAAAAAABgAAAAAAAAAEE1heFRva2Vuc1JlYWNoZWQAAAAZ",
        "AAAABQAAAChUb3BpY3MgYCgicGF1c2VkIiwgYnkpYCwgZGF0YSBgW2ZsYWdzXWAuAAAAAAAAAAZQYXVzZWQAAAAAAAEAAAAGcGF1c2VkAAAAAAACAAAAAAAAAAJieQAAAAAAEwAAAAEAAAAAAAAABWZsYWdzAAAAAAAABAAAAAAAAAAB",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAAFQAAAAAAAAA/TGVnYWN5OiBBZGRyZXNzIG9mIHRoZSBTRVAtNDEgdG9rZW4gdGhpcyBqYXIgYWNjZXB0cyAodjEgb25seSkuAAAAAAVUb2tlbgAAAAAAAAEAAAA+TGVnYWN5OiBXaXRoZHJhd2FibGUgYmFsYW5jZSBlc2Nyb3dlZCBmb3IgYSBjcmVhdG9yICh2MSBvbmx5KS4AAAAAAA5DcmVhdG9yQmFsYW5jZQAAAAAAAQAAABMAAAABAAAAgUhpc3RvcmljYWwgdG90YWwgZXZlciB0aXBwZWQgdG8gYSBjcmVhdG9yIChuZXZlciBkZWNyZWFzZXMpLiBUcmFja3MKdGhlIGdyb3NzIGFtb3VudCB0aXBwZWQsIGJlZm9yZSBhbnkgcHJvdG9jb2wgZmVlIGlzIGRlZHVjdGVkLgAAAAAAAAxDcmVhdG9yVG90YWwAAAABAAAAEwAAAAAAAAAoVG9rZW4gYWxsb3dsaXN0IGZvciBtdWx0aS10b2tlbiBzdXBwb3J0LgAAAA1BbGxvd2VkVG9rZW5zAAAAAAAAAQAAADpXaXRoZHJhd2FibGUgYmFsYW5jZSBlc2Nyb3dlZCBmb3IgYSAoY3JlYXRvciwgdG9rZW4pIHBhaXIuAAAAAAAHQmFsYW5jZQAAAAACAAAAEwAAABMAAAABAAAAOEhpc3RvcmljYWwgdG90YWwgZXZlciB0aXBwZWQgdG8gYSAoY3JlYXRvciwgdG9rZW4pIHBhaXIuAAAABVRvdGFsAAAAAAAAAgAAABMAAAATAAAAAQAAAChQYXlvdXQgYWRkcmVzcyBkZXNpZ25hdGVkIGZvciBhIGNyZWF0b3IuAAAADVBheW91dEFkZHJlc3MAAAAAAAABAAAAEwAAAAEAAABNUGVuZGluZyBjaGFuZ2UgdG8gcGF5b3V0IGFkZHJlc3M6IChjcmVhdG9yKSAtPiAobmV3X3BheW91dCwgZWZmZWN0aXZlX2xlZGdlcikAAAAAAAATUGVuZGluZ1BheW91dENoYW5nZQAAAAABAAAAEwAAAAEAAABGT3BlcmF0b3IgZGVsZWdhdGlvbjogKGNyZWF0b3IsIG9wZXJhdG9yKSAtPiAoYWxsb3dhbmNlLCBleHBpcnlfbGVkZ2VyKQAAAAAACE9wZXJhdG9yAAAAAgAAABMAAAATAAAAAAAAAJVBZGRyZXNzIGF1dGhvcml6ZWQgdG8gcHJvcG9zZS9jYW5jZWwgdXBncmFkZXMgYW5kIGFkbWluIHRyYW5zZmVycywgdG8KbWFuYWdlIHRoZSBjaXJjdWl0IGJyZWFrZXIgKHBhdXNlL2d1YXJkaWFuKSwgYW5kIHRvIGNvbmZpZ3VyZSB0aGUKcHJvdG9jb2wgZmVlLgAAAAAAAAVBZG1pbgAAAAAAAAAAAAA+VHdvLXN0ZXAgYWRtaW4gdHJhbnNmZXI6IGFkZHJlc3MgdGhhdCBtYXkgY2FsbCBgYWNjZXB0X2FkbWluYC4AAAAAAAxQZW5kaW5nQWRtaW4AAAAAAAAAVUxlZGdlciBkZWxheSBlbmZvcmNlZCBiZXR3ZWVuIGBwcm9wb3NlX3VwZ3JhZGVgIGFuZCBgZXhlY3V0ZV91cGdyYWRlYCwgc2V0IGF0IGBpbml0YC4AAAAAAAAWVXBncmFkZVRpbWVsb2NrTGVkZ2VycwAAAAAAAAAAADhQZW5kaW5nIHVwZ3JhZGUgcHJvcG9zYWw6IChuZXdfd2FzbV9oYXNoLCB1bmxvY2tfbGVkZ2VyKQAAAA5QZW5kaW5nVXBncmFkZQAAAAAAAAAAAEFTdG9yYWdlIHNjaGVtYSB2ZXJzaW9uLCBhZHZhbmNlZCBieSBgbWlncmF0ZSgpYCBhZnRlciBhbiB1cGdyYWRlLgAAAAAAAAtEYXRhVmVyc2lvbgAAAAAAAAAAd1NvbGUgZ3VhcmRpYW4gYWRkcmVzcywgc2V0dGFibGUgYnkgYWRtaW46IGNhbiBwYXVzZSBpbnN0YW50bHkgYnV0Cm5ldmVyIHVucGF1c2UuIEFic2VudCB1bnRpbCBgc2V0X2d1YXJkaWFuYCBpcyBjYWxsZWQuAAAAAAhHdWFyZGlhbgAAAAAAAAAoQ2lyY3VpdC1icmVha2VyIHN0YXRlLCBzZWUgYFBhdXNlU3RhdGVgLgAAAAVQYXVzZQAAAAAAAAAAAAA7Q29uZmlndXJhYmxlIGxlZGdlciBkdXJhdGlvbiBmb3IgZ3VhcmRpYW4taW5pdGlhdGVkIHBhdXNlcy4AAAAAFUd1YXJkaWFuUGF1c2VEdXJhdGlvbgAAAAAAAAAAAAA8UHJvdG9jb2wgZmVlIHJhdGUgaW4gYmFzaXMgcG9pbnRzLiBBYnNlbnQgb3IgMCBtZWFucyBubyBmZWUuAAAABkZlZUJwcwAAAAAAAAAAADVBZGRyZXNzIGF1dGhvcml6ZWQgdG8gd2l0aGRyYXcgYWNjcnVlZCBwcm90b2NvbCBmZWVzLgAAAAAAAAxGZWVDb2xsZWN0b3IAAAAAAAAAvExlZ2FjeSB1bnBhcmFtZXRlcml6ZWQgcHJvdG9jb2wgZmVlIGNvdW50ZXIuIEtlcHQgc28gZXhpc3RpbmcKcGVyc2lzdGVudCBlbnRyaWVzIHJlbWFpbiByZWFkYWJsZSBhZnRlciB1cGdyYWRlOyBtaWdyYXRlZCBpbnRvCmBGZWVCYWxhbmNlKHRva2VuKWAgZm9yIHRoZSBwcmltYXJ5IHRva2VuIG9uIGZpcnN0IGZlZSBhY2Nlc3MuAAAACkZlZUJhbGFuY2UAAAAAAAEAAADFV2l0aGRyYXdhYmxlIHByb3RvY29sIGZlZSBiYWxhbmNlIGFjY3J1ZWQgZnJvbSB0aXBzIGluIGB0b2tlbmAuCk5hbWVkIHRvIG1hdGNoIGBCYWxhbmNlYC9gVG90YWxgIChrZXllZCBieSB0b2tlbikgd2l0aG91dCBjb2xsaWRpbmcKd2l0aCB0aGUgbGVnYWN5IHVuaXQgYEZlZUJhbGFuY2VgIHZhcmlhbnQgcmVxdWlyZWQgZm9yIG1pZ3JhdGlvbi4AAAAAAAAPRmVlQmFsYW5jZVRva2VuAAAAAAEAAAAT",
        "AAAABQAAADpUb3BpY3MgYCgibWlncmF0ZWQiLClgLCBkYXRhIGAoZnJvbV92ZXJzaW9uLCB0b192ZXJzaW9uKWAuAAAAAAAAAAAACE1pZ3JhdGVkAAAAAQAAAAhtaWdyYXRlZAAAAAIAAAAAAAAADGZyb21fdmVyc2lvbgAAAAQAAAAAAAAAAAAAAAp0b192ZXJzaW9uAAAAAAAEAAAAAAAAAAE=",
        "AAAABQAAACpUb3BpY3MgYCgidW5wYXVzZWQiLCBieSlgLCBkYXRhIGBbZmxhZ3NdYC4AAAAAAAAAAAAIVW5wYXVzZWQAAAABAAAACHVucGF1c2VkAAAAAgAAAAAAAAACYnkAAAAAABMAAAABAAAAAAAAAAVmbGFncwAAAAAAAAQAAAAAAAAAAQ==",
        "AAAABQAAADtUb3BpY3MgYCgid2l0aGRyYXciLCBjcmVhdG9yKWAsIGRhdGEgYFt0b2tlbiwgYW1vdW50LCB0b11gLgAAAAAAAAAACFdpdGhkcmF3AAAAAQAAAAh3aXRoZHJhdwAAAAQAAAAAAAAAB2NyZWF0b3IAAAAAEwAAAAEAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAAAAAAAnRvAAAAAAATAAAAAAAAAAE=",
        "AAAAAQAAAxBDaXJjdWl0LWJyZWFrZXIgc3RhdGUsIHN0b3JlZCBhcyBhIHNpbmdsZSBpbnN0YW5jZS1zdG9yYWdlIGVudHJ5LgoKYGFkbWluX2ZsYWdzYCBhbmQgYGd1YXJkaWFuX2ZsYWdzYCBhcmUgaW5kZXBlbmRlbnQgYml0bWFza3Mgb3ZlcgpgUEFVU0VfRkxBR18qYCByYXRoZXIgdGhhbiBzZXBhcmF0ZSBib29sZWFucywgc28gdGlwcy93aXRoZHJhd2FscyBwYXVzZQppbmRlcGVuZGVudGx5LiBUaGV5J3JlIGtlcHQgaW4gdHdvIGJ1Y2tldHMgKHJhdGhlciB0aGFuIG9uZSBzaGFyZWQKYml0bWFzaykgc28gYSBndWFyZGlhbidzIHRlbXBvcmFyeSBwYXVzZSBjYW4gbmV2ZXIgc2lsZW50bHkgb3ZlcndyaXRlLCBvcgpiZSBzaWxlbnRseSBwcm9tb3RlZCBpbnRvLCBhbiBhZG1pbidzIGRlbGliZXJhdGUgcGVyc2lzdGVudCBwYXVzZToKLSBgYWRtaW5fZmxhZ3NgIGJpdHMgYXJlIHNldCBvbmx5IGJ5IHRoZSBhZG1pbiBhbmQgbmV2ZXIgYXV0by1leHBpcmU7Cm9ubHkgYW4gYWRtaW4gYHVucGF1c2VfKmAgY2FsbCBjbGVhcnMgdGhlbS4KLSBgZ3VhcmRpYW5fZmxhZ3NgIGJpdHMgYXJlIHNldCBvbmx5IGJ5IHRoZSBndWFyZGlhbiBhbmQgYXV0by1leHBpcmUgYXQKYGd1YXJkaWFuX2V4cGlyeWAgKGEgc2luZ2xlIHNoYXJlZCBsZWRnZXIgY2hlY2twb2ludCkgdW5sZXNzIHRoZSBhZG1pbgpjb25maXJtcyB0aGVtIGZpcnN0IGJ5IGNhbGxpbmcgdGhlIG1hdGNoaW5nIGBwYXVzZV8qYCwgd2hpY2ggcHJvbW90ZXMKdGhlbSBpbnRvIGBhZG1pbl9mbGFnc2AgYW5kIGNsZWFycyB0aGVtIGhlcmUuAAAAAAAAAApQYXVzZVN0YXRlAAAAAAADAAAAAAAAAAthZG1pbl9mbGFncwAAAAAEAAAAAAAAAA9ndWFyZGlhbl9leHBpcnkAAAAABAAAAAAAAAAOZ3VhcmRpYW5fZmxhZ3MAAAAAAAQ=",
        "AAAABQAAAPZUb3BpY3MgYCgiZmVlX2NoYXJnZWQiLCBjcmVhdG9yKWAsIGRhdGEgYFtncm9zcywgZmVlLCBuZXRdYC4KRW1pdHRlZCBhbG9uZ3NpZGUgYFRpcGAgd2hlbmV2ZXIgYSBub256ZXJvIGZlZSByYXRlIGlzIGNvbmZpZ3VyZWQsIHNvIHRoZQppbmRleGVyIGNhbiByZWNvbnN0cnVjdCBhY2NvdW50aW5nIHdpdGhvdXQgcmUtZGVyaXZpbmcgdGhlIGZlZSBzY2hlZHVsZQp0aGF0IHdhcyBhY3RpdmUgYXQgdGhlIHRpbWUgb2YgdGhlIHRpcC4AAAAAAAAAAAAKRmVlQ2hhcmdlZAAAAAAAAQAAAAtmZWVfY2hhcmdlZAAAAAAEAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAAAAAAAAVncm9zcwAAAAAAAAsAAAAAAAAAAAAAAANmZWUAAAAACwAAAAAAAAAAAAAAA25ldAAAAAALAAAAAAAAAAE=",
        "AAAABQAAADZUb3BpY3MgYCgiZmVlX3dpdGhkcmF3IiwgY29sbGVjdG9yKWAsIGRhdGEgYFthbW91bnRdYC4AAAAAAAAAAAALRmVlV2l0aGRyYXcAAAAAAQAAAAxmZWVfd2l0aGRyYXcAAAACAAAAAAAAAAljb2xsZWN0b3IAAAAAAAATAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAE=",
        "AAAABQAAADxUb3BpY3MgYCgiZmVlX2NvbmZpZ3VyZWQiLCBhZG1pbilgLCBkYXRhIGBbYnBzLCBjb2xsZWN0b3JdYC4AAAAAAAAADUZlZUNvbmZpZ3VyZWQAAAAAAAABAAAADmZlZV9jb25maWd1cmVkAAAAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAAAAAAAANicHMAAAAABAAAAAAAAAAAAAAACWNvbGxlY3RvcgAAAAAAABMAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAD0d1YXJkaWFuVXBkYXRlZAAAAAABAAAAEGd1YXJkaWFuX3VwZGF0ZWQAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAAAAAAAAhndWFyZGlhbgAAABMAAAAAAAAAAQ==",
        "AAAABQAAAAAAAAAAAAAAD09wZXJhdG9yUmV2b2tlZAAAAAABAAAAEG9wZXJhdG9yX3Jldm9rZWQAAAACAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAAAAAAAAhvcGVyYXRvcgAAABMAAAABAAAAAQ==",
        "AAAABQAAAC9Ub3BpY3MgYCgidXBncmFkZV9leGVjdXRlZCIsIGhhc2gpYCwgZGF0YSBgKClgLgAAAAAAAAAAD1VwZ3JhZGVFeGVjdXRlZAAAAAABAAAAEHVwZ3JhZGVfZXhlY3V0ZWQAAAABAAAAAAAAAARoYXNoAAAD7gAAACAAAAABAAAAAQ==",
        "AAAABQAAAD1Ub3BpY3MgYCgidXBncmFkZV9wcm9wb3NlZCIsIGhhc2gpYCwgZGF0YSBgKHVubG9ja19sZWRnZXIsKWAuAAAAAAAAAAAAAA9VcGdyYWRlUHJvcG9zZWQAAAAAAQAAABB1cGdyYWRlX3Byb3Bvc2VkAAAAAgAAAAAAAAAEaGFzaAAAA+4AAAAgAAAAAQAAAAAAAAANdW5sb2NrX2xlZGdlcgAAAAAAAAQAAAAAAAAAAQ==",
        "AAAABQAAADBUb3BpY3MgYCgidXBncmFkZV9jYW5jZWxsZWQiLCBoYXNoKWAsIGRhdGEgYCgpYC4AAAAAAAAAEFVwZ3JhZGVDYW5jZWxsZWQAAAABAAAAEXVwZ3JhZGVfY2FuY2VsbGVkAAAAAAAAAQAAAAAAAAAEaGFzaAAAA+4AAAAgAAAAAQAAAAE=",
        "AAAABQAAAAAAAAAAAAAAEk9wZXJhdG9yQXV0aG9yaXplZAAAAAAAAQAAABNvcGVyYXRvcl9hdXRob3JpemVkAAAAAAQAAAAAAAAAB2NyZWF0b3IAAAAAEwAAAAEAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAEAAAAAAAAACWFsbG93YW5jZQAAAAAAAAsAAAAAAAAAAAAAAA1leHBpcnlfbGVkZ2VyAAAAAAAABAAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAE1BheW91dENoYW5nZUFwcGxpZWQAAAAAAQAAABVwYXlvdXRfY2hhbmdlX2FwcGxpZWQAAAAAAAACAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAAAAAAAApuZXdfcGF5b3V0AAAAAAATAAAAAAAAAAE=",
        "AAAABQAAAAAAAAAAAAAAFFBheW91dENoYW5nZVByb3Bvc2VkAAAAAQAAABZwYXlvdXRfY2hhbmdlX3Byb3Bvc2VkAAAAAAADAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAAAAAAAApuZXdfcGF5b3V0AAAAAAATAAAAAAAAAAAAAAAQZWZmZWN0aXZlX2xlZGdlcgAAAAQAAAAAAAAAAQ==",
        "AAAABQAAADtUb3BpY3MgYCgiYWRtaW5fdHJhbnNmZXJfYWNjZXB0ZWQiLClgLCBkYXRhIGAobmV3X2FkbWluLClgLgAAAAAAAAAAFUFkbWluVHJhbnNmZXJBY2NlcHRlZAAAAAAAAAEAAAAXYWRtaW5fdHJhbnNmZXJfYWNjZXB0ZWQAAAAAAQAAAAAAAAAJbmV3X2FkbWluAAAAAAAAEwAAAAAAAAAB",
        "AAAABQAAAElUb3BpY3MgYCgiYWRtaW5fdHJhbnNmZXJfcHJvcG9zZWQiLClgLCBkYXRhIGAoY3VycmVudF9hZG1pbiwgbmV3X2FkbWluKWAuAAAAAAAAAAAAABVBZG1pblRyYW5zZmVyUHJvcG9zZWQAAAAAAAABAAAAF2FkbWluX3RyYW5zZmVyX3Byb3Bvc2VkAAAAAAIAAAAAAAAADWN1cnJlbnRfYWRtaW4AAAAAAAATAAAAAAAAAAAAAAAJbmV3X2FkbWluAAAAAAAAEwAAAAAAAAAB",
        "AAAABQAAAAAAAAAAAAAAFVBheW91dENoYW5nZUNhbmNlbGxlZAAAAAAAAAEAAAAXcGF5b3V0X2NoYW5nZV9jYW5jZWxsZWQAAAAAAQAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAQAAAAE=",
        "AAAABQAAADhUb3BpY3MgYCgiYWRtaW5fdHJhbnNmZXJfY2FuY2VsbGVkIiwgYWRtaW4pYCwgZGF0YSBgW11gLgAAAAAAAAAWQWRtaW5UcmFuc2ZlckNhbmNlbGxlZAAAAAAAAQAAABhhZG1pbl90cmFuc2Zlcl9jYW5jZWxsZWQAAAABAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAAAQ==",
        "AAAAAAAAATBFc2Nyb3dzIGBhbW91bnRgIG9mIHRoZSBjb25maWd1cmVkIHRva2VuIGZyb20gYHNlbmRlcmAgZm9yIGBjcmVhdG9yYCwKbGVzcyB0aGUgcHJvdG9jb2wgZmVlIChpZiBvbmUgaXMgY29uZmlndXJlZCkuIFRoZSBjcmVhdG9yJ3MgYmFsYW5jZSBpcwpjcmVkaXRlZCB3aXRoIGBhbW91bnQgLSBmZWVgOyB0aGUgZmVlIGl0c2VsZiBhY2NydWVzIHRvCmBGZWVCYWxhbmNlKHRva2VuKWAgZm9yIGxhdGVyIHdpdGhkcmF3YWwgYnkgdGhlIGZlZSBjb2xsZWN0b3IuCmBmZWUgKyBuZXQgPT0gYW1vdW50YCBob2xkcyBmb3IgZXZlcnkgaW5wdXQuAAAAA3RpcAAAAAAEAAAAAAAAAAZzZW5kZXIAAAAAABMAAAAAAAAAB2NyZWF0b3IAAAAAEwAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAA",
        "AAAAAAAAAN9PbmUtdGltZSBjb25maWd1cmF0aW9uOiB0aGUgdG9rZW4gdGhpcyBqYXIgYWNjZXB0cyAoc2VlZGVkIGFzIHRoZQpmaXJzdCBlbnRyeSBvZiB0aGUgbXVsdGktdG9rZW4gYWxsb3dsaXN0KSwgdGhlIGFkbWluLCBhbmQgdGhlIGxlZGdlcgpkZWxheSBgZXhlY3V0ZV91cGdyYWRlYCBtdXN0IHdhaXQgb3V0IGFmdGVyIGEgYHByb3Bvc2VfdXBncmFkZWAuCkVycm9ycyBpZiBjYWxsZWQgdHdpY2UuAAAAAARpbml0AAAAAwAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAAGHVwZ3JhZGVfdGltZWxvY2tfbGVkZ2VycwAAAAQAAAAA",
        "AAAAAAAAAVZBZG1pbi1vbmx5LCBpZGVtcG90ZW50LiBBZHZhbmNlcyBgRGF0YUtleTo6RGF0YVZlcnNpb25gIHRvd2FyZHMgdGhpcwpidWlsZCdzIGBEQVRBX1ZFUlNJT05gLCBhcHBseWluZyBhbnkgc3RvcmFnZSB0cmFuc2Zvcm1hdGlvbiB0aGUgbmV3CldBU00gcmVxdWlyZXMuIEEgbm8tb3AgKG5vIHBhbmljLCBubyBldmVudCkgaWYgdGhlIHN0b3JlZCB2ZXJzaW9uCmFscmVhZHkgbWVldHMgb3IgZXhjZWVkcyBgREFUQV9WRVJTSU9OYCDigJQgc2FmZSB0byBjYWxsIG1vcmUgdGhhbiBvbmNlLAppbmNsdWRpbmcgYmVmb3JlIHRoZSBmaXJzdCB1cGdyYWRlIG9yIGFmdGVyIGEgcmVwZWF0ZWQgaW52b2NhdGlvbi4AAAAAAAdtaWdyYXRlAAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAA=",
        "AAAAAAAAAIJTZXRzIHRoZSBwcm90b2NvbCBmZWUgcmF0ZSBhbmQgaXRzIGNvbGxlY3Rvci4gQWRtaW4tb25seTsgYGJwc2AgaXMKaGFyZC1jYXBwZWQgYXQgYE1BWF9GRUVfQlBTYC4gU2V0dGluZyBgYnBzYCB0byAwIGRpc2FibGVzIGZlZXMuAAAAAAAHc2V0X2ZlZQAAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAAA2JwcwAAAAAEAAAAAAAAAAljb2xsZWN0b3IAAAAAAAATAAAAAA==",
        "AAAAAAAAAD9QYXlzIG91dCBhIGNyZWF0b3IncyB3aXRoZHJhd2FibGUgYmFsYW5jZSBmb3IgYSBzcGVjaWZpYyB0b2tlbi4AAAAACHdpdGhkcmF3AAAABQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAdjcmVhdG9yAAAAABMAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAACdG8AAAAAABMAAAAAAAAABmFtb3VudAAAAAAD6AAAAAsAAAAA",
        "AAAAAAAAADRBZGRzIGEgdG9rZW4gdG8gdGhlIGFsbG93bGlzdC4gQWRtaW4tb25seSBvcGVyYXRpb24uAAAACWFkZF90b2tlbgAAAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAA==",
        "AAAAAAAAABZDdXJyZW50IGFkbWluIGFkZHJlc3MuAAAAAAAJZ2V0X2FkbWluAAAAAAAAAAAAAAEAAAAT",
        "AAAAAAAAAD9QYXVzZXMgYm90aCB0aXBzIGFuZCB3aXRoZHJhd2FscyBpbiBvbmUgY2FsbC4gU2VlIGBwYXVzZV90aXBzYC4AAAAACXBhdXNlX2FsbAAAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAACNSZXR1cm5zIHRoZSBsaXN0IG9mIGFsbG93ZWQgdG9rZW5zLgAAAAAKZ2V0X3Rva2VucwAAAAAAAAAAAAEAAAPqAAAAEw==",
        "AAAAAAAAAItQYXVzZXMgYHRpcGAuIENhbGxhYmxlIGJ5IGFkbWluIChwZXJzaXN0cyB1bnRpbCBleHBsaWNpdGx5IHVucGF1c2VkKQpvciBndWFyZGlhbiAoYXV0by1leHBpcmVzOyBjYWxsIGFnYWluIGFzIGFkbWluIHRvIGNvbmZpcm0vcGVyc2lzdCBpdCkuAAAAAApwYXVzZV90aXBzAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAKdGlwX2xlZ2FjeQAAAAAAAwAAAAAAAAAGc2VuZGVyAAAAAAATAAAAAAAAAAdjcmVhdG9yAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAA=",
        "AAAAAAAAAD9HZXRzIHRoZSB3aXRoZHJhd2FibGUgYmFsYW5jZSBmb3IgYSBjcmVhdG9yIGFuZCBzcGVjaWZpYyB0b2tlbi4AAAAAC2dldF9iYWxhbmNlAAAAAAIAAAAAAAAAB2NyZWF0b3IAAAAAEwAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAALZ2V0X2ZlZV9icHMAAAAAAAAAAAEAAAAE",
        "AAAAAAAAAExUaGUgY3VycmVudGx5IGVmZmVjdGl2ZSBwYXVzZSBiaXRtYXNrIChhY2NvdW50aW5nIGZvciBndWFyZGlhbiBhdXRvLWV4cGlyeSkuAAAAC3BhdXNlX2ZsYWdzAAAAAAAAAAABAAAABA==",
        "AAAAAAAAALlDb21wdXRlcyBgKGZlZSwgbmV0KWAgZm9yIGBhbW91bnRgIGF0IGBicHNgIHdpdGhvdXQgdG91Y2hpbmcgc3RvcmFnZS4KRXhwb3NlZCByZWFkLW9ubHkgc28gb2ZmLWNoYWluIGNhbGxlcnMgKFNES3MsIGluZGV4ZXJzLCB0ZXN0cykgY2FuCnByZXZpZXcgdGhlIGV4YWN0IHNwbGl0IHRoZSBjb250cmFjdCB3aWxsIGFwcGx5LgAAAAAAAAtwcmV2aWV3X2ZlZQAAAAACAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAA2JwcwAAAAAEAAAAAQAAA+0AAAACAAAACwAAAAs=",
        "AAAAAAAAADtVbnBhdXNlcyBib3RoIHRpcHMgYW5kIHdpdGhkcmF3YWxzIGluIG9uZSBjYWxsLiBBZG1pbiBvbmx5LgAAAAALdW5wYXVzZV9hbGwAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAA==",
        "AAAAAAAAAGFDb21wbGV0ZXMgYSB0d28tc3RlcCBhZG1pbiB0cmFuc2Zlci4gTXVzdCBiZSBjYWxsZWQgYnkgdGhlIGFkZHJlc3MKbmFtZWQgaW4gdGhlIHBlbmRpbmcgcHJvcG9zYWwuAAAAAAAADGFjY2VwdF9hZG1pbgAAAAEAAAAAAAAACW5ld19hZG1pbgAAAAAAABMAAAAA",
        "AAAAAAAAAAAAAAAMZ2V0X2d1YXJkaWFuAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAAAAAAAGBSZW1vdmVzIGEgdG9rZW4gZnJvbSB0aGUgYWxsb3dsaXN0LiBBZG1pbi1vbmx5IG9wZXJhdGlvbi4KRXhpc3RpbmcgYmFsYW5jZXMgcmVtYWluIHdpdGhkcmF3YWJsZS4AAAAMcmVtb3ZlX3Rva2VuAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAA",
        "AAAAAAAAADBBcHBvaW50cyAob3IgcmVwbGFjZXMpIHRoZSBndWFyZGlhbi4gQWRtaW4gb25seS4AAAAMc2V0X2d1YXJkaWFuAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAhndWFyZGlhbgAAABMAAAAA",
        "AAAAAAAAAEVVbnBhdXNlcyBgdGlwYC4gQWRtaW4gb25seSDigJQgZ3VhcmRpYW5zIGNhbiBwYXVzZSBidXQgbmV2ZXIgdW5wYXVzZS4AAAAAAAAMdW5wYXVzZV90aXBzAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAA==",
        "AAAAAAAAAMxQcm9wb3NlcyBgbmV3X2FkbWluYCBhcyB0aGUgbmV4dCBhZG1pbi4gVGFrZXMgZWZmZWN0IG9ubHkgb25jZQpgbmV3X2FkbWluYCBjYWxscyBgYWNjZXB0X2FkbWluYCDigJQgYSBzaW5nbGUtc3RlcCB0cmFuc2ZlciB0byBhIHR5cG8nZApvciB1bnJlYWNoYWJsZSBhZGRyZXNzIGNhbiBuZXZlciBwZXJtYW5lbnRseSBsb2NrIG91dCBhZG1pbmlzdHJhdGlvbi4AAAANcHJvcG9zZV9hZG1pbgAAAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAJbmV3X2FkbWluAAAAAAAAEwAAAAA=",
        "AAAAAAAAAJ5QYXlzIG91dCB0aGUgZmVlIGNvbGxlY3RvcidzIGZ1bGwgb3IgcGFydGlhbCBzaGFyZSBvZgpgRmVlQmFsYW5jZSh0b2tlbilgLiBPbmx5IG1vdmVzIHVuaXRzIG9mIGB0b2tlbmAuIE1pcnJvcnMgYHdpdGhkcmF3YCdzCnBhdHRlcm4sIGluY2x1ZGluZyBUVEwgZXh0ZW5zaW9uLgAAAAAADXdpdGhkcmF3X2ZlZXMAAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAPoAAAACwAAAAA=",
        "AAAAAAAAAE9BZG1pbi1vbmx5LiBBYm9ydHMgYSBwZW5kaW5nIHVwZ3JhZGUgcHJvcG9zYWwgd2l0aG91dCB3YWl0aW5nIG91dCB0aGUKdGltZWxvY2suAAAAAA5jYW5jZWxfdXBncmFkZQAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAA==",
        "AAAAAAAAAFdIaXN0b3JpY2FsIHRvdGFsIGV2ZXIgdGlwcGVkIHRvIGBjcmVhdG9yYCBmb3IgYSBzcGVjaWZpYyBgdG9rZW5gLiBaZXJvIGlmIG5ldmVyIHRpcHBlZC4AAAAADmdldF90b3RhbF90aXBzAAAAAAACAAAAAAAAAAdjcmVhdG9yAAAAABMAAAAAAAAABXRva2VuAAAAAAAAEwAAAAEAAAAL",
        "AAAAAAAAAZdTd2FwcyB0aGlzIGNvbnRyYWN0J3MgV0FTTSB0byB0aGUgcHJvcG9zZWQgaGFzaCBvbmNlIGl0cyB0aW1lbG9jayBoYXMKZWxhcHNlZC4gUGVybWlzc2lvbmxlc3MgYnkgZGVzaWduIOKAlCB0aGUgYWRtaW4gYWxyZWFkeSBhdXRob3JpemVkIHRoZQp1cGdyYWRlIGF0IGBwcm9wb3NlX3VwZ3JhZGVgLCBhbmQgaXRzIHVubG9jayBsZWRnZXIgaXMgcHVibGljCm9uLWNoYWluIHN0YXRlLCBzbyBubyBjYWxsZXIgaWRlbnRpdHkgY2hlY2sgYWRkcyBtZWFuaW5nZnVsIHNlY3VyaXR5CmhlcmUuIFN0b3JhZ2UgaXMgcHJlc2VydmVkIGJ5IHRoZSBob3N0IGFjcm9zcyB0aGUgc3dhcDsgY2FsbCB0aGUgbmV3CldBU00ncyBgbWlncmF0ZSgpYCBhZnRlcndhcmRzIHRvIGFwcGx5IGFueSBzdG9yYWdlLWxheW91dCBjaGFuZ2VzLgAAAAAPZXhlY3V0ZV91cGdyYWRlAAAAAAAAAAAA",
        "AAAAAAAAAAAAAAAPZ2V0X2ZlZV9iYWxhbmNlAAAAAAEAAAAAAAAABXRva2VuAAAAAAAAEwAAAAEAAAAL",
        "AAAAAAAAAMtBZG1pbi1vbmx5LiBSZWNvcmRzIGBuZXdfd2FzbV9oYXNoYCBhcyBhIHBlbmRpbmcgdXBncmFkZSwgdW5sb2NrZWQKYWZ0ZXIgdGhlIGxlZGdlciBkZWxheSBjb25maWd1cmVkIGF0IGBpbml0YC4gT25seSBvbmUgcHJvcG9zYWwgbWF5IGJlCnBlbmRpbmcgYXQgYSB0aW1lIOKAlCBjYW5jZWwgdGhlIGV4aXN0aW5nIG9uZSBmaXJzdCB0byByZXBsYWNlIGl0LgAAAAAPcHJvcG9zZV91cGdyYWRlAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAANbmV3X3dhc21faGFzaAAAAAAAA+4AAAAgAAAAAA==",
        "AAAAAAAAAAAAAAAPcmV2b2tlX29wZXJhdG9yAAAAAAIAAAAAAAAAB2NyZWF0b3IAAAAAEwAAAAAAAAAIb3BlcmF0b3IAAAATAAAAAA==",
        "AAAAAAAAAAAAAAAPd2l0aGRyYXdfbGVnYWN5AAAAAAQAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAPoAAAACwAAAAA=",
        "AAAAAAAAAB9DdXJyZW50IHN0b3JhZ2Ugc2NoZW1hIHZlcnNpb24uAAAAABBnZXRfZGF0YV92ZXJzaW9uAAAAAAAAAAEAAAAE",
        "AAAAAAAAAAAAAAARZ2V0X2ZlZV9jb2xsZWN0b3IAAAAAAAAAAAAAAQAAA+gAAAAT",
        "AAAAAAAAADVBZGRyZXNzIGN1cnJlbnRseSBwcm9wb3NlZCBhcyB0aGUgbmV4dCBhZG1pbiwgaWYgYW55LgAAAAAAABFnZXRfcGVuZGluZ19hZG1pbgAAAAAAAAAAAAABAAAD6AAAABM=",
        "AAAAAAAAAFZUcnVlIGlmIGV2ZXJ5IGJpdCBpbiBgZmxhZ2AgaXMgY3VycmVudGx5IHBhdXNlZCAoYWNjb3VudGluZyBmb3IKZ3VhcmRpYW4gYXV0by1leHBpcnkpLgAAAAAAEWlzX2ZlYXR1cmVfcGF1c2VkAAAAAAAAAQAAAAAAAAAEZmxhZwAAAAQAAAABAAAAAQ==",
        "AAAAAAAAAE1QYXVzZXMgYHdpdGhkcmF3YCBhbmQgdGhlIHdpdGhkcmF3YWwtbWVjaGFuaWNzIGVudHJ5cG9pbnRzLiBTZWUgYHBhdXNlX3RpcHNgLgAAAAAAABFwYXVzZV93aXRoZHJhd2FscwAAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAASYXV0aG9yaXplX29wZXJhdG9yAAAAAAAEAAAAAAAAAAdjcmVhdG9yAAAAABMAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAAAAAAJYWxsb3dhbmNlAAAAAAAACwAAAAAAAAANZXhwaXJ5X2xlZGdlcgAAAAAAAAQAAAAA",
        "AAAAAAAAAAAAAAASc2V0X3BheW91dF9hZGRyZXNzAAAAAAACAAAAAAAAAAdjcmVhdG9yAAAAABMAAAAAAAAABnBheW91dAAAAAAAEwAAAAA=",
        "AAAAAAAAAElVbnBhdXNlcyBgd2l0aGRyYXdgIGFuZCB0aGUgd2l0aGRyYXdhbC1tZWNoYW5pY3MgZW50cnlwb2ludHMuIEFkbWluIG9ubHkuAAAAAAAAE3VucGF1c2Vfd2l0aGRyYXdhbHMAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAA==",
        "AAAAAAAAAFJBYmFuZG9ucyBhIHBlbmRpbmcgYWRtaW4gdHJhbnNmZXIsIGxlYXZpbmcgdGhlIGN1cnJlbnQgYWRtaW4gaW4KcGxhY2UuIEFkbWluLW9ubHkuAAAAAAAVY2FuY2VsX2FkbWluX3RyYW5zZmVyAAAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAA==",
        "AAAAAAAAAAAAAAAVY2FuY2VsX3BheW91dF9hZGRyZXNzAAAAAAAAAQAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAA==",
        "AAAAAAAAAAAAAAAVZ2V0X3RvdGFsX3RpcHNfbGVnYWN5AAAAAAAAAQAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAQAAAAs=",
        "AAAAAAAAAF5Db25maWd1cmVzIGhvdyBtYW55IGxlZGdlcnMgYSBndWFyZGlhbi1pbml0aWF0ZWQgcGF1c2UgbGFzdHMgYmVmb3JlCmF1dG8tZXhwaXJpbmcuIEFkbWluIG9ubHkuAAAAAAAbc2V0X2d1YXJkaWFuX3BhdXNlX2R1cmF0aW9uAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAHbGVkZ2VycwAAAAAEAAAAAA==",
        "AAAAAAAAAG1MZWRnZXIgc2VxdWVuY2UgYXQgd2hpY2ggdGhlIGN1cnJlbnQgZ3VhcmRpYW4tb3JpZ2luYXRlZCBwYXVzZSBiaXRzCmV4cGlyZS4gMCBpZiBubyBndWFyZGlhbiBwYXVzZSBpcyBhY3RpdmUuAAAAAAAAHGd1YXJkaWFuX3BhdXNlX2V4cGlyeV9sZWRnZXIAAAAAAAAAAQAAAAQ=" ]),
      options
    )
  }
  public readonly fromJSON = {
    tip: this.txFromJSON<null>,
        init: this.txFromJSON<null>,
        migrate: this.txFromJSON<null>,
        set_fee: this.txFromJSON<null>,
        withdraw: this.txFromJSON<null>,
        add_token: this.txFromJSON<null>,
        get_admin: this.txFromJSON<string>,
        pause_all: this.txFromJSON<null>,
        get_tokens: this.txFromJSON<Array<string>>,
        pause_tips: this.txFromJSON<null>,
        tip_legacy: this.txFromJSON<null>,
        get_balance: this.txFromJSON<i128>,
        get_fee_bps: this.txFromJSON<u32>,
        pause_flags: this.txFromJSON<u32>,
        preview_fee: this.txFromJSON<readonly [i128, i128]>,
        unpause_all: this.txFromJSON<null>,
        accept_admin: this.txFromJSON<null>,
        get_guardian: this.txFromJSON<Option<string>>,
        remove_token: this.txFromJSON<null>,
        set_guardian: this.txFromJSON<null>,
        unpause_tips: this.txFromJSON<null>,
        propose_admin: this.txFromJSON<null>,
        withdraw_fees: this.txFromJSON<null>,
        cancel_upgrade: this.txFromJSON<null>,
        get_total_tips: this.txFromJSON<i128>,
        execute_upgrade: this.txFromJSON<null>,
        get_fee_balance: this.txFromJSON<i128>,
        propose_upgrade: this.txFromJSON<null>,
        revoke_operator: this.txFromJSON<null>,
        withdraw_legacy: this.txFromJSON<null>,
        get_data_version: this.txFromJSON<u32>,
        get_fee_collector: this.txFromJSON<Option<string>>,
        get_pending_admin: this.txFromJSON<Option<string>>,
        is_feature_paused: this.txFromJSON<boolean>,
        pause_withdrawals: this.txFromJSON<null>,
        authorize_operator: this.txFromJSON<null>,
        set_payout_address: this.txFromJSON<null>,
        unpause_withdrawals: this.txFromJSON<null>,
        cancel_admin_transfer: this.txFromJSON<null>,
        cancel_payout_address: this.txFromJSON<null>,
        get_total_tips_legacy: this.txFromJSON<i128>,
        set_guardian_pause_duration: this.txFromJSON<null>,
        guardian_pause_expiry_ledger: this.txFromJSON<u32>
  }
}
