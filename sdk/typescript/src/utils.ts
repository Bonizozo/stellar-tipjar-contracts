export const getRpcUrl = (network: 'testnet' | 'mainnet'): string => {
  return network === 'testnet'
    ? 'https://soroban-testnet.stellar.org'
    : 'https://soroban.stellar.org';
};

/**
 * Retries an async function with exponential backoff.
 *
 * @param fn - Async function to retry.
 * @param retries - Number of retry attempts (default 3).
 * @param delay - Base delay in ms, doubles on each retry (default 1000).
 * @returns Resolved value from fn.
 * @throws Last encountered error after all retries are exhausted.
 */
export async function retry<T>(
  fn: () => Promise<T>,
  retries = 3,
  delay = 1000
): Promise<T> {
  let lastError: unknown;

  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error;
      if (attempt < retries) {
        await new Promise((r) => setTimeout(r, delay * 2 ** attempt));
      }
    }
  }

  throw lastError;
}
