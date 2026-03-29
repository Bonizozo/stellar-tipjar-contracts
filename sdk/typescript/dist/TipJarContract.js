"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.TipJarContract = void 0;
const stellar_sdk_1 = require("@stellar/stellar-sdk");
const utils_1 = require("./utils");
class TipJarContract {
    /**
     * Creates a new TipJarContract instance.
     *
     * @param contractId - Bech32m contract address (C…).
     * @param network - Target Stellar network ('testnet' | 'mainnet').
     */
    constructor(contractId, network) {
        this.contract = new stellar_sdk_1.Contract(contractId);
        this.server = new stellar_sdk_1.rpc.Server((0, utils_1.getRpcUrl)(network));
        this.networkPassphrase =
            network === 'testnet' ? stellar_sdk_1.Networks.TESTNET : stellar_sdk_1.Networks.PUBLIC;
    }
    /* ===============================
       WALLET: Freighter Sign
    ================================ */
    /**
     * Signs a transaction XDR using the Freighter browser extension.
     *
     * @param xdr - Unsigned transaction XDR string.
     * @returns Signed transaction XDR string.
     * @throws If Freighter is not installed or the user rejects signing.
     */
    async signWithFreighter(xdr) {
        if (!window.freighterApi) {
            throw new Error('Freighter wallet not installed. Visit https://www.freighter.app');
        }
        const result = await window.freighterApi.signTransaction(xdr, {
            networkPassphrase: this.networkPassphrase,
        });
        if (!result) {
            throw new Error('Freighter returned an empty signed transaction.');
        }
        return result;
    }
    /* ===============================
       WAIT FOR TX CONFIRMATION
    ================================ */
    /**
     * Polls the RPC node until a transaction is confirmed or fails.
     *
     * @param hash - Transaction hash to poll.
     * @returns The confirmed transaction response.
     * @throws If the transaction fails or is never confirmed.
     */
    async waitForConfirmation(hash) {
        let attempts = 0;
        const maxAttempts = 30;
        while (attempts < maxAttempts) {
            const tx = await this.server.getTransaction(hash);
            if (tx.status === 'SUCCESS')
                return tx;
            if (tx.status === 'FAILED') {
                throw new Error(`Transaction failed on-chain: ${hash}`);
            }
            // NOT_FOUND — keep polling
            await new Promise((r) => setTimeout(r, 1500));
            attempts++;
        }
        throw new Error(`Transaction ${hash} was not confirmed after ${maxAttempts} attempts.`);
    }
    /* ===============================
       HANDLE TX RESPONSE
    ================================ */
    /**
     * Validates a send response and waits for on-chain confirmation.
     *
     * @param result - Response from server.sendTransaction().
     * @returns Resolved TipResult with txHash and ledger.
     * @throws If the transaction was not accepted as PENDING.
     */
    async handleTxResponse(result) {
        if (result.status === 'ERROR') {
            throw new Error(`Transaction rejected by network: ${result.errorResult?.toXDR('base64') ?? 'unknown error'}`);
        }
        if (result.status !== 'PENDING') {
            throw new Error(`Unexpected transaction status: ${result.status}`);
        }
        const finalTx = await this.waitForConfirmation(result.hash);
        return {
            success: true,
            txHash: result.hash,
            ledger: finalTx.ledger ?? 0,
        };
    }
    /* ===============================
       SEND TIP
    ================================ */
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
    async sendTip(params) {
        if (!params.creator.startsWith('G')) {
            throw new Error('Invalid creator address — must be a Stellar public key (G…)');
        }
        if (!params.tipper.startsWith('G')) {
            throw new Error('Invalid tipper address — must be a Stellar public key (G…)');
        }
        return (0, utils_1.retry)(async () => {
            const account = await this.server.getAccount(params.tipper);
            const tx = new stellar_sdk_1.TransactionBuilder(account, {
                fee: stellar_sdk_1.BASE_FEE,
                networkPassphrase: this.networkPassphrase,
            })
                .addOperation(this.contract.call('send_tip', (0, stellar_sdk_1.nativeToScVal)(params.creator, { type: 'address' }), (0, stellar_sdk_1.nativeToScVal)(params.amount, { type: 'i128' }), (0, stellar_sdk_1.nativeToScVal)(params.tipper, { type: 'address' }), (0, stellar_sdk_1.nativeToScVal)(params.memo || '', { type: 'string' })))
                .setTimeout(30)
                .build();
            // Simulate to populate auth entries and resource fee
            const simulation = await this.server.simulateTransaction(tx);
            if (!stellar_sdk_1.rpc.Api.isSimulationSuccess(simulation)) {
                throw new Error(`sendTip simulation failed: ${simulation
                    .error}`);
            }
            const assembledTx = stellar_sdk_1.rpc.assembleTransaction(tx, simulation).build();
            const signedXdr = await this.signWithFreighter(assembledTx.toXDR());
            const signedTx = stellar_sdk_1.TransactionBuilder.fromXDR(signedXdr, this.networkPassphrase);
            const result = await this.server.sendTransaction(signedTx);
            return this.handleTxResponse(result);
        });
    }
    /* ===============================
       GET BALANCE
    ================================ */
    /**
     * Returns the current tip balance for a creator account, in stroops.
     *
     * @param creator - Stellar public key (G…) of the creator.
     * @returns Balance in stroops as a bigint.
     * @throws If simulation fails or the return value cannot be decoded.
     */
    async getBalance(creator) {
        const account = await this.server.getAccount(creator);
        const tx = new stellar_sdk_1.TransactionBuilder(account, {
            fee: stellar_sdk_1.BASE_FEE,
            networkPassphrase: this.networkPassphrase,
        })
            .addOperation(this.contract.call('get_balance', (0, stellar_sdk_1.nativeToScVal)(creator, { type: 'address' })))
            .setTimeout(30)
            .build();
        const simulation = await this.server.simulateTransaction(tx);
        if (!stellar_sdk_1.rpc.Api.isSimulationSuccess(simulation)) {
            throw new Error(`getBalance simulation failed: ${simulation.error}`);
        }
        const retval = simulation.result?.retval;
        if (!retval) {
            throw new Error('getBalance returned no value from contract.');
        }
        return (0, stellar_sdk_1.scValToNative)(retval);
    }
    /* ===============================
       WITHDRAW
    ================================ */
    /**
     * Withdraws a specified amount from the contract to the creator's account.
     *
     * @param creator - Stellar public key (G…) of the creator.
     * @param amount - Amount in stroops to withdraw.
     * @returns WithdrawResult with txHash and ledger.
     * @throws If simulation fails, signing is rejected, or the transaction fails.
     */
    async withdraw(creator, amount) {
        if (!creator.startsWith('G')) {
            throw new Error('Invalid creator address — must be a Stellar public key (G…)');
        }
        return (0, utils_1.retry)(async () => {
            const account = await this.server.getAccount(creator);
            const tx = new stellar_sdk_1.TransactionBuilder(account, {
                fee: stellar_sdk_1.BASE_FEE,
                networkPassphrase: this.networkPassphrase,
            })
                .addOperation(this.contract.call('withdraw', (0, stellar_sdk_1.nativeToScVal)(creator, { type: 'address' }), (0, stellar_sdk_1.nativeToScVal)(amount, { type: 'i128' })))
                .setTimeout(30)
                .build();
            // Simulate to populate auth entries and resource fee
            const simulation = await this.server.simulateTransaction(tx);
            if (!stellar_sdk_1.rpc.Api.isSimulationSuccess(simulation)) {
                throw new Error(`withdraw simulation failed: ${simulation
                    .error}`);
            }
            const assembledTx = stellar_sdk_1.rpc.assembleTransaction(tx, simulation).build();
            const signedXdr = await this.signWithFreighter(assembledTx.toXDR());
            const signedTx = stellar_sdk_1.TransactionBuilder.fromXDR(signedXdr, this.networkPassphrase);
            const result = await this.server.sendTransaction(signedTx);
            return this.handleTxResponse(result);
        });
    }
}
exports.TipJarContract = TipJarContract;
