import {
  Contract,
  TransactionBuilder,
  Networks,
  BASE_FEE,
  nativeToScVal,
  scValToNative,
  rpc as SorobanRpc,
} from '@stellar/stellar-sdk';

import { SendTipParams, TipResult, WithdrawResult } from './types';
import { getRpcUrl, retry } from './utils';

export class TipJarContract {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;

  /**
   * Creates a new TipJarContract instance.
   *
   * @param contractId - Bech32m contract address (C…).
   * @param network - Target Stellar network ('testnet' | 'mainnet').
   */
  constructor(contractId: string, network: 'testnet' | 'mainnet') {
    this.contract = new Contract(contractId);
    this.server = new SorobanRpc.Server(getRpcUrl(network));
    this.networkPassphrase =
      network === 'testnet' ? Networks.TESTNET : Networks.PUBLIC;
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
  async signWithFreighter(xdr: string): Promise<string> {
    if (!(window as any).freighterApi) {
      throw new Error(
        'Freighter wallet not installed. Visit https://www.freighter.app'
      );
    }

    const result = await (window as any).freighterApi.signTransaction(xdr, {
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
  async waitForConfirmation(
    hash: string
  ): Promise<SorobanRpc.Api.GetTransactionResponse> {
    let attempts = 0;
    const maxAttempts = 30;

    while (attempts < maxAttempts) {
      const tx = await this.server.getTransaction(hash);

      if (tx.status === 'SUCCESS') return tx;
      if (tx.status === 'FAILED') {
        throw new Error(`Transaction failed on-chain: ${hash}`);
      }

      // NOT_FOUND — keep polling
      await new Promise((r) => setTimeout(r, 1500));
      attempts++;
    }

    throw new Error(
      `Transaction ${hash} was not confirmed after ${maxAttempts} attempts.`
    );
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
  private async handleTxResponse(
    result: SorobanRpc.Api.SendTransactionResponse
  ): Promise<TipResult> {
    if (result.status === 'ERROR') {
      throw new Error(
        `Transaction rejected by network: ${
          result.errorResult?.toXDR('base64') ?? 'unknown error'
        }`
      );
    }

    if (result.status !== 'PENDING') {
      throw new Error(`Unexpected transaction status: ${result.status}`);
    }

    const finalTx = await this.waitForConfirmation(result.hash);

    return {
      success: true,
      txHash: result.hash,
      ledger: (finalTx as any).ledger ?? 0,
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
  async sendTip(params: SendTipParams): Promise<TipResult> {
    if (!params.creator.startsWith('G')) {
      throw new Error(
        'Invalid creator address — must be a Stellar public key (G…)'
      );
    }
    if (!params.tipper.startsWith('G')) {
      throw new Error(
        'Invalid tipper address — must be a Stellar public key (G…)'
      );
    }

    return retry(async () => {
      const account = await this.server.getAccount(params.tipper);

      const tx = new TransactionBuilder(account, {
        fee: BASE_FEE,
        networkPassphrase: this.networkPassphrase,
      })
        .addOperation(
          this.contract.call(
            'send_tip',
            nativeToScVal(params.creator, { type: 'address' }),
            nativeToScVal(params.amount, { type: 'i128' }),
            nativeToScVal(params.tipper, { type: 'address' }),
            nativeToScVal(params.memo || '', { type: 'string' })
          )
        )
        .setTimeout(30)
        .build();

      // Simulate to populate auth entries and resource fee
      const simulation = await this.server.simulateTransaction(tx);

      if (!SorobanRpc.Api.isSimulationSuccess(simulation)) {
        throw new Error(
          `sendTip simulation failed: ${
            (simulation as SorobanRpc.Api.SimulateTransactionErrorResponse)
              .error
          }`
        );
      }

      const assembledTx = SorobanRpc.assembleTransaction(
        tx,
        simulation
      ).build();

      const signedXdr = await this.signWithFreighter(assembledTx.toXDR());

      const signedTx = TransactionBuilder.fromXDR(
        signedXdr,
        this.networkPassphrase
      );

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
  async getBalance(creator: string): Promise<bigint> {
    const account = await this.server.getAccount(creator);

    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          'get_balance',
          nativeToScVal(creator, { type: 'address' })
        )
      )
      .setTimeout(30)
      .build();

    const simulation = await this.server.simulateTransaction(tx);

    if (!SorobanRpc.Api.isSimulationSuccess(simulation)) {
      throw new Error(
        `getBalance simulation failed: ${
          (simulation as SorobanRpc.Api.SimulateTransactionErrorResponse).error
        }`
      );
    }

    const retval = simulation.result?.retval;
    if (!retval) {
      throw new Error('getBalance returned no value from contract.');
    }

    return scValToNative(retval) as bigint;
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
  async withdraw(creator: string, amount: bigint): Promise<WithdrawResult> {
    if (!creator.startsWith('G')) {
      throw new Error(
        'Invalid creator address — must be a Stellar public key (G…)'
      );
    }

    return retry(async () => {
      const account = await this.server.getAccount(creator);

      const tx = new TransactionBuilder(account, {
        fee: BASE_FEE,
        networkPassphrase: this.networkPassphrase,
      })
        .addOperation(
          this.contract.call(
            'withdraw',
            nativeToScVal(creator, { type: 'address' }),
            nativeToScVal(amount, { type: 'i128' })
          )
        )
        .setTimeout(30)
        .build();

      // Simulate to populate auth entries and resource fee
      const simulation = await this.server.simulateTransaction(tx);

      if (!SorobanRpc.Api.isSimulationSuccess(simulation)) {
        throw new Error(
          `withdraw simulation failed: ${
            (simulation as SorobanRpc.Api.SimulateTransactionErrorResponse)
              .error
          }`
        );
      }

      const assembledTx = SorobanRpc.assembleTransaction(
        tx,
        simulation
      ).build();

      const signedXdr = await this.signWithFreighter(assembledTx.toXDR());

      const signedTx = TransactionBuilder.fromXDR(
        signedXdr,
        this.networkPassphrase
      );

      const result = await this.server.sendTransaction(signedTx);

      return this.handleTxResponse(result) as unknown as WithdrawResult;
    });
  }
}