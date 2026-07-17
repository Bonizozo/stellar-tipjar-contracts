import { Network } from './types';
import { NetworkError } from './errors';

export { parseTipEvent, parseWithdrawEvent } from './events';

export const NETWORK_CONFIG: Record<Network, { rpcUrl: string; networkPassphrase: string }> = {
  testnet: {
    rpcUrl: 'https://soroban-testnet.stellar.org',
    networkPassphrase: 'Test SDF Network ; September 2015',
  },
  mainnet: {
    rpcUrl: 'https://soroban.stellar.org',
    networkPassphrase: 'Public Global Stellar Network ; September 2015',
  },
};

export async function withRetry<T>(fn: () => Promise<T>, retries = 3): Promise<T> {
  let attempt = 0;
  while (true) {
    try {
      return await fn();
    } catch (err) {
      attempt++;
      if (attempt > retries) {
        throw new NetworkError(
          `Operation failed after ${retries} retries: ${(err as Error).message}`,
          retries,
        );
      }
      await new Promise((r) => setTimeout(r, 2 ** attempt * 200));
    }
  }
}
