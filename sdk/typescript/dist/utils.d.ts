export declare const getRpcUrl: (network: "testnet" | "mainnet") => string;
/**
 * Retries an async function with exponential backoff.
 *
 * @param fn - Async function to retry.
 * @param retries - Number of retry attempts (default 3).
 * @param delay - Base delay in ms, doubles on each retry (default 1000).
 * @returns Resolved value from fn.
 * @throws Last encountered error after all retries are exhausted.
 */
export declare function retry<T>(fn: () => Promise<T>, retries?: number, delay?: number): Promise<T>;
//# sourceMappingURL=utils.d.ts.map