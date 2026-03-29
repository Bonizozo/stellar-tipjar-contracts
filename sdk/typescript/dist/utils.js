"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.getRpcUrl = void 0;
exports.retry = retry;
const getRpcUrl = (network) => {
    return network === 'testnet'
        ? 'https://soroban-testnet.stellar.org'
        : 'https://soroban.stellar.org';
};
exports.getRpcUrl = getRpcUrl;
/**
 * Retries an async function with exponential backoff.
 *
 * @param fn - Async function to retry.
 * @param retries - Number of retry attempts (default 3).
 * @param delay - Base delay in ms, doubles on each retry (default 1000).
 * @returns Resolved value from fn.
 * @throws Last encountered error after all retries are exhausted.
 */
async function retry(fn, retries = 3, delay = 1000) {
    let lastError;
    for (let attempt = 0; attempt <= retries; attempt++) {
        try {
            return await fn();
        }
        catch (error) {
            lastError = error;
            if (attempt < retries) {
                await new Promise((r) => setTimeout(r, delay * 2 ** attempt));
            }
        }
    }
    throw lastError;
}
