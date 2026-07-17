import { TipJarContract } from '../src/TipJarContract';
import { InvalidAmountError, TransactionFailedError, ContractNotInitializedError } from '../src/errors';
import { parseTipEvent, parseWithdrawEvent } from '../src/events';
import { NETWORK_CONFIG } from '../src/utils';
import { TipJarSDK } from '../src/client';

// ── Mocks ─────────────────────────────────────────────────────────────────────

jest.mock('@stellar/stellar-sdk', () => {
  const mockSimulateOk = {
    result: { retval: 'MOCK_RETVAL' },
  };
  const mockSendResult = { status: 'PENDING', hash: 'mock-hash' };
  const mockGetTxResult = { status: 'SUCCESS' };

  return {
    Contract: jest.fn().mockImplementation(() => ({
      call: jest.fn().mockReturnValue('mock-operation'),
    })),
    Keypair: {
      fromSecret: jest.fn().mockReturnValue({
        publicKey: () => 'GPUBLIC',
        sign: jest.fn(),
      }),
    },
    Networks: { TESTNET: 'Test SDF Network ; September 2015' },
    SorobanRpc: {
      Server: jest.fn().mockImplementation(() => ({
        getAccount: jest.fn().mockResolvedValue({ id: 'GPUBLIC', sequence: '1' }),
        simulateTransaction: jest.fn().mockResolvedValue(mockSimulateOk),
        sendTransaction: jest.fn().mockResolvedValue(mockSendResult),
        getTransaction: jest.fn().mockResolvedValue(mockGetTxResult),
        getEvents: jest.fn().mockResolvedValue({ events: [] }),
      })),
      Api: {
        isSimulationError: jest.fn().mockReturnValue(false),
        GetTransactionStatus: { NOT_FOUND: 'NOT_FOUND', FAILED: 'FAILED', SUCCESS: 'SUCCESS' },
      },
      assembleTransaction: jest.fn().mockReturnValue({
        build: jest.fn().mockReturnValue({ sign: jest.fn(), toXDR: jest.fn() }),
      }),
    },
    TransactionBuilder: jest.fn().mockImplementation(() => ({
      addOperation: jest.fn().mockReturnThis(),
      setTimeout: jest.fn().mockReturnThis(),
      build: jest.fn().mockReturnValue({ sign: jest.fn(), toXDR: jest.fn() }),
    })),
    nativeToScVal: jest.fn((v) => v),
    scValToNative: jest.fn((v) => v),
    xdr: {},
  };
});

// ── Network config ────────────────────────────────────────────────────────────

describe('NETWORK_CONFIG', () => {
  it('has testnet and mainnet entries', () => {
    expect(NETWORK_CONFIG.testnet.rpcUrl).toBe('https://soroban-testnet.stellar.org');
    expect(NETWORK_CONFIG.mainnet.rpcUrl).toBe('https://soroban.stellar.org');
  });

  it('has correct network passphrases', () => {
    expect(NETWORK_CONFIG.testnet.networkPassphrase).toContain('Test SDF');
    expect(NETWORK_CONFIG.mainnet.networkPassphrase).toContain('Public Global');
  });
});

// ── Error classes ─────────────────────────────────────────────────────────────

describe('Error classes', () => {
  it('InvalidAmountError has correct name', () => {
    const e = new InvalidAmountError();
    expect(e.name).toBe('InvalidAmountError');
    expect(e).toBeInstanceOf(Error);
  });

  it('TransactionFailedError stores txHash', () => {
    const e = new TransactionFailedError('failed', 'abc123');
    expect(e.txHash).toBe('abc123');
    expect(e.name).toBe('TransactionFailedError');
  });

  it('ContractNotInitializedError has correct name', () => {
    const e = new ContractNotInitializedError();
    expect(e.name).toBe('ContractNotInitializedError');
  });
});

// ── Event parsing ─────────────────────────────────────────────────────────────

describe('parseTipEvent', () => {
  it('extracts sender and amount', () => {
    const mockEvent = { value: ['GSENDER', '500'] } as any;
    const result = parseTipEvent(mockEvent);
    expect(result.sender).toBe('GSENDER');
    expect(result.amount).toBe(500n);
  });
});

describe('parseWithdrawEvent', () => {
  it('extracts amount', () => {
    const mockEvent = { value: ['250'] } as any;
    const result = parseWithdrawEvent(mockEvent);
    expect(result.amount).toBe(250n);
  });
});

// ── TipJarContract ────────────────────────────────────────────────────────────

describe('TipJarContract', () => {
  const config = { contractId: 'CCONTRACT', network: 'testnet' as const };
  const { Keypair } = jest.requireMock('@stellar/stellar-sdk');
  const keypair = Keypair.fromSecret('SECRET');

  it('throws InvalidAmountError for zero amount', async () => {
    const sdk = new TipJarContract(config);
    sdk.connect(keypair);
    await expect(
      sdk.sendTip({ tipper: 'GA', creator: 'GB', amount: 0n }),
    ).rejects.toBeInstanceOf(InvalidAmountError);
  });

  it('throws InvalidAmountError for negative amount', async () => {
    const sdk = new TipJarContract(config);
    sdk.connect(keypair);
    await expect(
      sdk.sendTip({ tipper: 'GA', creator: 'GB', amount: -1n }),
    ).rejects.toBeInstanceOf(InvalidAmountError);
  });

  it('sendTip returns txHash on success', async () => {
    const sdk = new TipJarContract(config);
    sdk.connect(keypair);
    const result = await sdk.sendTip({ tipper: 'GA', creator: 'GB', amount: 100n });
    expect(result.txHash).toBe('mock-hash');
    expect(result.creator).toBe('GB');
    expect(result.amount).toBe(100n);
  });

  it('getTipEvents returns empty array when no events', async () => {
    const sdk = new TipJarContract(config);
    sdk.connect(keypair);
    const events = await sdk.getTipEvents('GB');
    expect(events).toEqual([]);
  });

  it('throws TransactionFailedError without keypair', async () => {
    const sdk = new TipJarContract(config);
    await expect(
      sdk.sendTip({ tipper: 'GA', creator: 'GB', amount: 100n }),
    ).rejects.toBeInstanceOf(TransactionFailedError);
  });
});

// ── TipJarSDK alias ───────────────────────────────────────────────────────────

describe('TipJarSDK alias', () => {
  it('is the same class as TipJarContract', () => {
    expect(TipJarSDK).toBe(TipJarContract);
  });
});
