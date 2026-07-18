import {
  Contract,
  Keypair,
  Networks,
  SorobanRpc,
  TransactionBuilder,
  nativeToScVal,
  scValToNative,
  xdr,
} from '@stellar/stellar-sdk';
import {
  SdkConfig,
  TipParams,
  TipResult,
  WithdrawResult,
  TipEvent,
  Network,
} from './types';
import {
  InvalidAmountError,
  TransactionFailedError,
  ContractNotInitializedError,
} from './errors';
import { NETWORK_CONFIG, parseTipEvent, parseWithdrawEvent, withRetry } from './utils';

const BASE_FEE = '100';
const TX_TIMEOUT_SEC = 30;

export class TipJarContract {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;
  private keypair: Keypair | null = null;

  constructor(private readonly config: SdkConfig) {
    const net = NETWORK_CONFIG[config.network];
    this.contract = new Contract(config.contractId);
    this.server = new SorobanRpc.Server(config.rpcUrl ?? net.rpcUrl);
    this.networkPassphrase = net.networkPassphrase;
  }

  /**
   * Store a keypair for transaction signing.
   * @param keypair - Stellar Keypair used to sign transactions.
   * @example
   * sdk.connect(Keypair.fromSecret(process.env.SECRET!));
   */
  connect(keypair: Keypair): void {
    this.keypair = keypair;
  }

  /**
   * Send a tip to a creator.
   * @param params - Tip parameters including creator, amount, and tipper addresses.
   * @returns TipResult with transaction hash, creator, and amount.
   * @throws InvalidAmountError if amount is not positive.
   * @throws TransactionFailedError if the transaction fails.
   * @example
   * const result = await sdk.sendTip({ creator: 'G...', amount: 100n, tipper: 'G...' });
   */
  async sendTip(params: TipParams): Promise<TipResult> {
    if (params.amount <= 0n) throw new InvalidAmountError();
    const op = this.contract.call(
      'tip',
      nativeToScVal(params.tipper, { type: 'address' }),
      nativeToScVal(params.creator, { type: 'address' }),
      nativeToScVal(params.amount, { type: 'i128' }),
    );
    const txHash = await withRetry(() => this.buildAndSubmit(op));
    return { txHash, creator: params.creator, amount: params.amount };
  }

  /**
   * Withdraw the full escrowed balance for a creator.
   * @param creator - Stellar address of the creator.
   * @returns WithdrawResult with transaction hash, creator, and withdrawn amount.
   * @throws TransactionFailedError if the transaction fails.
   * @example
   * const result = await sdk.withdraw('G...');
   */
  async withdraw(creator: string): Promise<WithdrawResult> {
    const balanceBefore = await this.getBalance(creator);
    const op = this.contract.call('withdraw', nativeToScVal(creator, { type: 'address' }));
    const txHash = await withRetry(() => this.buildAndSubmit(op));
    return { txHash, creator, amount: balanceBefore };
  }

  /**
   * Get the total historical tips received by a creator.
   * @param creator - Stellar address of the creator.
   * @returns Total tips as bigint.
   * @throws ContractNotInitializedError if the contract has no data for this creator.
   * @example
   * const total = await sdk.getTotalTips('G...');
   */
  async getTotalTips(creator: string): Promise<bigint> {
    return withRetry(async () => {
      const result = await this.server.simulateTransaction(
        await this.buildReadTx(
          this.contract.call('get_total_tips', nativeToScVal(creator, { type: 'address' })),
        ),
      );
      if (SorobanRpc.Api.isSimulationError(result)) {
        throw new ContractNotInitializedError(result.error);
      }
      return BigInt(scValToNative((result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result!.retval) as string);
    });
  }

  /**
   * Get the withdrawable (escrowed) balance for a creator.
   * Alias that reads CreatorBalance via get_total_tips simulation pattern.
   * @param creator - Stellar address of the creator.
   * @returns Withdrawable balance as bigint.
   * @throws ContractNotInitializedError if the contract has no data for this creator.
   * @example
   * const balance = await sdk.getBalance('G...');
   */
  async getBalance(creator: string): Promise<bigint> {
    // The contract exposes withdrawable balance through ledger storage;
    // we simulate a withdraw call to read CreatorBalance without submitting.
    return withRetry(async () => {
      const result = await this.server.simulateTransaction(
        await this.buildReadTx(
          this.contract.call('withdraw', nativeToScVal(creator, { type: 'address' })),
        ),
      );
      if (SorobanRpc.Api.isSimulationError(result)) {
        throw new ContractNotInitializedError(result.error);
      }
      return BigInt(scValToNative((result as SorobanRpc.Api.SimulateTransactionSuccessResponse).result!.retval) as string);
    });
  }

  /**
   * Fetch on-chain tip events for a creator.
   * @param creator - Stellar address of the creator.
   * @param limit - Maximum number of events to return (default 20).
   * @returns Array of TipEvent objects.
   * @example
   * const events = await sdk.getTipEvents('G...', 10);
   */
  async getTipEvents(creator: string, limit = 20): Promise<TipEvent[]> {
    return withRetry(async () => {
      const { events } = await this.server.getEvents({
        filters: [{ type: 'contract', contractIds: [this.config.contractId], topics: [['tip', creator]] }],
        limit,
      });
      return events.map(parseTipEvent);
    });
  }

  // ── Private ────────────────────────────────────────────────────────────────

  private async buildReadTx(operation: xdr.Operation): Promise<ReturnType<TransactionBuilder['build']>> {
    const account = await this.server.getAccount(
      this.keypair?.publicKey() ?? this.config.contractId,
    );
    return new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(TX_TIMEOUT_SEC)
      .build();
  }

  private async buildAndSubmit(operation: xdr.Operation): Promise<string> {
    if (!this.keypair) throw new TransactionFailedError('Call connect() with a Keypair before submitting transactions');

    const account = await this.server.getAccount(this.keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(TX_TIMEOUT_SEC)
      .build();

    const simResult = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(simResult)) {
      throw new TransactionFailedError(`Simulation failed: ${simResult.error}`);
    }

    const preparedTx = SorobanRpc.assembleTransaction(tx, simResult).build();
    preparedTx.sign(this.keypair);

    const sendResult = await this.server.sendTransaction(preparedTx);
    if (sendResult.status === 'ERROR') {
      throw new TransactionFailedError(`Submit failed: ${sendResult.errorResult?.toXDR('base64')}`, sendResult.hash);
    }

    // Poll for confirmation.
    let getResult = await this.server.getTransaction(sendResult.hash);
    for (let i = 0; i < 10 && getResult.status === SorobanRpc.Api.GetTransactionStatus.NOT_FOUND; i++) {
      await new Promise((r) => setTimeout(r, 1000));
      getResult = await this.server.getTransaction(sendResult.hash);
    }
    if (getResult.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
      throw new TransactionFailedError('Transaction failed on-chain', sendResult.hash);
    }

    return sendResult.hash;
  }
}
