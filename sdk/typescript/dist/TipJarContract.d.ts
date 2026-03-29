import { rpc as SorobanRpc } from '@stellar/stellar-sdk';
import { SendTipParams, TipResult, WithdrawResult } from './types';
export declare class TipJarContract {
    private contract;
    private server;
    private networkPassphrase;
    /**
     * Creates a new TipJarContract instance.
     *
     * @param contractId - Bech32m contract address (C…).
     * @param network - Target Stellar network ('testnet' | 'mainnet').
     */
    constructor(contractId: string, network: 'testnet' | 'mainnet');
    /**
     * Signs a transaction XDR using the Freighter browser extension.
     *
     * @param xdr - Unsigned transaction XDR string.
     * @returns Signed transaction XDR string.
     * @throws If Freighter is not installed or the user rejects signing.
     */
    signWithFreighter(xdr: string): Promise<string>;
    /**
     * Polls the RPC node until a transaction is confirmed or fails.
     *
     * @param hash - Transaction hash to poll.
     * @returns The confirmed transaction response.
     * @throws If the transaction fails or is never confirmed.
     */
    waitForConfirmation(hash: string): Promise<SorobanRpc.Api.GetTransactionResponse>;
    /**
     * Validates a send response and waits for on-chain confirmation.
     *
     * @param result - Response from server.sendTransaction().
     * @returns Resolved TipResult with txHash and ledger.
     * @throws If the transaction was not accepted as PENDING.
     */
    private handleTxResponse;
    /**
     * Sends a tip from a tipper to a creator.
     *
     * Builds the transaction, simulates it to populate resource fees and
     * auth entries, signs via Freighter, then submits and awaits confirmation.
     *
     * @param params - Tip parameters (creator, tipper, amount, optional memo).
     * @returns TipResult with txHash and ledger.
     * @throws If the address is invalid, simulation fails, or signing is rejected.
     */
    sendTip(params: SendTipParams): Promise<TipResult>;
    /**
     * Returns the current tip balance for a creator account, in stroops.
     *
     * @param creator - Stellar public key (G…) of the creator.
     * @returns Balance in stroops as a bigint.
     * @throws If simulation fails or the return value cannot be decoded.
     */
    getBalance(creator: string): Promise<bigint>;
    /**
     * Withdraws a specified amount from the contract to the creator's account.
     *
     * @param creator - Stellar public key (G…) of the creator.
     * @param amount - Amount in stroops to withdraw.
     * @returns WithdrawResult with txHash and ledger.
     * @throws If simulation fails, signing is rejected, or the transaction fails.
     */
    withdraw(creator: string, amount: bigint): Promise<WithdrawResult>;
}
//# sourceMappingURL=TipJarContract.d.ts.map