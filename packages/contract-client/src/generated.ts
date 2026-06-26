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
  4: {message:"NothingToWithdraw"}
}

export type DataKey = {tag: "Token", values: void} | {tag: "CreatorBalance", values: readonly [string]} | {tag: "CreatorTotal", values: readonly [string]};


export interface Client {
  /**
   * Construct and simulate a tip transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Escrows `amount` of the configured token from `sender` for `creator`.
   */
  tip: ({sender, creator, amount}: {sender: string, creator: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a init transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time configuration of the token this jar accepts. Errors if called twice.
   */
  init: ({token}: {token: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pays out a creator's full withdrawable balance and resets it to zero.
   * Historical totals are left untouched.
   */
  withdraw: ({creator}: {creator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_total_tips transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Historical total ever tipped to `creator`. Zero if the creator has never been tipped.
   */
  get_total_tips: ({creator}: {creator: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

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
      new ContractSpec([ "AAAAAAAAAEVFc2Nyb3dzIGBhbW91bnRgIG9mIHRoZSBjb25maWd1cmVkIHRva2VuIGZyb20gYHNlbmRlcmAgZm9yIGBjcmVhdG9yYC4AAAAAAAADdGlwAAAAAAMAAAAAAAAABnNlbmRlcgAAAAAAEwAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAA",
        "AAAABQAAADNUb3BpY3MgYCgidGlwIiwgY3JlYXRvcilgLCBkYXRhIGAoc2VuZGVyLCBhbW91bnQpYC4AAAAAAAAAAANUaXAAAAAAAQAAAAN0aXAAAAAAAwAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAQAAAAAAAAAGc2VuZGVyAAAAAAATAAAAAAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAE=",
        "AAAAAAAAAE1PbmUtdGltZSBjb25maWd1cmF0aW9uIG9mIHRoZSB0b2tlbiB0aGlzIGphciBhY2NlcHRzLiBFcnJvcnMgaWYgY2FsbGVkIHR3aWNlLgAAAAAAAARpbml0AAAAAQAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAA==",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABAAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAMAAAAAAAAAEU5vdGhpbmdUb1dpdGhkcmF3AAAAAAAABA==",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAAAwAAAAAAAAAtQWRkcmVzcyBvZiB0aGUgU0VQLTQxIHRva2VuIHRoaXMgamFyIGFjY2VwdHMuAAAAAAAABVRva2VuAAAAAAAAAQAAACxXaXRoZHJhd2FibGUgYmFsYW5jZSBlc2Nyb3dlZCBmb3IgYSBjcmVhdG9yLgAAAA5DcmVhdG9yQmFsYW5jZQAAAAAAAQAAABMAAAABAAAAPEhpc3RvcmljYWwgdG90YWwgZXZlciB0aXBwZWQgdG8gYSBjcmVhdG9yIChuZXZlciBkZWNyZWFzZXMpLgAAAAxDcmVhdG9yVG90YWwAAAABAAAAEw==",
        "AAAAAAAAAGtQYXlzIG91dCBhIGNyZWF0b3IncyBmdWxsIHdpdGhkcmF3YWJsZSBiYWxhbmNlIGFuZCByZXNldHMgaXQgdG8gemVyby4KSGlzdG9yaWNhbCB0b3RhbHMgYXJlIGxlZnQgdW50b3VjaGVkLgAAAAAId2l0aGRyYXcAAAABAAAAAAAAAAdjcmVhdG9yAAAAABMAAAAA",
        "AAAABQAAAC5Ub3BpY3MgYCgid2l0aGRyYXciLCBjcmVhdG9yKWAsIGRhdGEgYGFtb3VudGAuAAAAAAAAAAAACFdpdGhkcmF3AAAAAQAAAAh3aXRoZHJhdwAAAAIAAAAAAAAAB2NyZWF0b3IAAAAAEwAAAAEAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAA",
        "AAAAAAAAAFVIaXN0b3JpY2FsIHRvdGFsIGV2ZXIgdGlwcGVkIHRvIGBjcmVhdG9yYC4gWmVybyBpZiB0aGUgY3JlYXRvciBoYXMgbmV2ZXIgYmVlbiB0aXBwZWQuAAAAAAAADmdldF90b3RhbF90aXBzAAAAAAABAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAACw==" ]),
      options
    )
  }
  public readonly fromJSON = {
    tip: this.txFromJSON<null>,
        init: this.txFromJSON<null>,
        withdraw: this.txFromJSON<null>,
        get_total_tips: this.txFromJSON<i128>
  }
}