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
import { CONTRACT_IDS, Network } from './networks';

const BASE_FEE = '100';
const TX_TIMEOUT_SEC = 30;

export interface ClientConfig {
  contractId: string;
  network: Network;
  rpcUrl?: string;
}

export interface TipParams {
  sender: string;
  creator: string;
  amount: bigint;
  memo?: string;
}

export interface TipResult {
  txHash: string;
  creator: string;
  amount: bigint;
}

export interface WithdrawResult {
  txHash: string;
  creator: string;
  amount: bigint;
}

export interface TipEvent {
  sender: string;
  amount: bigint;
}

export interface ClientOptions {
  contractId?: string;
  network?: Network;
  rpcUrl?: string;
}

export class Client {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private networkPassphrase: string;
  private keypair: Keypair | null = null;

  constructor(config: ClientConfig) {
    const network = config.network;
    const rpcUrl = config.rpcUrl ?? this.defaultRpcUrl(network);
    const networkPassphrase = this.defaultNetworkPassphrase(network);

    this.contract = new Contract(config.contractId);
    this.server = new SorobanRpc.Server(rpcUrl);
    this.networkPassphrase = networkPassphrase;
  }

  connect(keypair: Keypair): void {
    this.keypair = keypair;
  }

  async tip(params: TipParams): Promise<TipResult> {
    if (params.amount <= 0n) {
      throw new Error('Tip amount must be positive.');
    }

    const op = this.contract.call(
      'tip',
      nativeToScVal(params.sender, { type: 'address' }),
      nativeToScVal(params.creator, { type: 'address' }),
      nativeToScVal(params.amount, { type: 'i128' }),
    );

    const txHash = await this.buildAndSubmit(op);
    return { txHash, creator: params.creator, amount: params.amount };
  }

  async withdraw(creator: string): Promise<WithdrawResult> {
    const op = this.contract.call('withdraw', nativeToScVal(creator, { type: 'address' }));
    const txHash = await this.buildAndSubmit(op);
    return { txHash, creator, amount: 0n };
  }

  async getTotalTips(creator: string): Promise<bigint> {
    const result = await this.server.simulateTransaction(
      await this.buildReadTx(
        this.contract.call('get_total_tips', nativeToScVal(creator, { type: 'address' })),
      ),
    );
    return BigInt(scValToNative((result as any).result!.retval) as string);
  }

  async getBalance(creator: string): Promise<bigint> {
    const result = await this.server.simulateTransaction(
      await this.buildReadTx(
        this.contract.call('withdraw', nativeToScVal(creator, { type: 'address' })),
      ),
    );
    return BigInt(scValToNative((result as any).result!.retval) as string);
  }

  private async buildReadTx(operation: xdr.Operation): Promise<ReturnType<TransactionBuilder['build']>> {
    const account = await this.server.getAccount(this.keypair?.publicKey() ?? 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF');
    return new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(TX_TIMEOUT_SEC)
      .build();
  }

  private async buildAndSubmit(operation: xdr.Operation): Promise<string> {
    if (!this.keypair) {
      throw new Error('Call connect() with a Keypair before submitting transactions.');
    }

    const account = await this.server.getAccount(this.keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(TX_TIMEOUT_SEC)
      .build();

    const simResult = await this.server.simulateTransaction(tx);
    const preparedTx = SorobanRpc.assembleTransaction(tx, simResult).build();
    preparedTx.sign(this.keypair);

    const sendResult = await this.server.sendTransaction(preparedTx);
    if (sendResult.status === 'ERROR') {
      throw new Error('Transaction failed.');
    }

    let getResult = await this.server.getTransaction(sendResult.hash);
    for (let i = 0; i < 10 && getResult.status === SorobanRpc.Api.GetTransactionStatus.NOT_FOUND; i += 1) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
      getResult = await this.server.getTransaction(sendResult.hash);
    }

    return sendResult.hash;
  }

  private defaultRpcUrl(network: Network): string {
    return network === 'mainnet' ? 'https://soroban.stellar.org' : 'https://soroban-testnet.stellar.org';
  }

  private defaultNetworkPassphrase(network: Network): string {
    return network === 'mainnet'
      ? 'Public Global Stellar Network ; September 2015'
      : 'Test SDF Network ; September 2015';
  }
}
