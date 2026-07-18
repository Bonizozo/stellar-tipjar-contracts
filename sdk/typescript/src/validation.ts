/**
 * Validation schemas and helpers for TipJar SDK inputs.
 *
 * Provides typed validation for tip parameters, creator addresses,
 * and asset codes before they are submitted to the contract.
 */

export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

/** Minimum tip amount in stroops (1 stroop). */
export const MIN_TIP_AMOUNT = 1n;

/** Maximum tip message length in characters. */
export const MAX_MESSAGE_LENGTH = 280;

/** Maximum tip batch size. */
export const MAX_BATCH_SIZE = 50;

/** Stellar public key regex (G... 56 chars). */
const STELLAR_ADDRESS_RE = /^G[A-Z2-7]{55}$/;

/** Stellar asset code regex (1–12 alphanumeric chars). */
const ASSET_CODE_RE = /^[A-Za-z0-9]{1,12}$/;

/**
 * Validate a Stellar address (G... public key or contract address C...).
 */
export function validateAddress(address: string): ValidationResult {
  const errors: string[] = [];
  if (!address || typeof address !== 'string') {
    errors.push('Address is required');
  } else if (!/^[GC][A-Z2-7]{55}$/.test(address)) {
    errors.push('Address must be a valid Stellar public key (G...) or contract address (C...)');
  }
  return { valid: errors.length === 0, errors };
}

/**
 * Validate a tip amount.
 * @param amount - Amount in stroops (bigint).
 */
export function validateTipAmount(amount: bigint): ValidationResult {
  const errors: string[] = [];
  if (typeof amount !== 'bigint') {
    errors.push('Amount must be a bigint');
  } else if (amount < MIN_TIP_AMOUNT) {
    errors.push(`Amount must be at least ${MIN_TIP_AMOUNT} stroop`);
  }
  return { valid: errors.length === 0, errors };
}

/**
 * Validate a tip message.
 * @param message - Optional message string.
 */
export function validateMessage(message: string): ValidationResult {
  const errors: string[] = [];
  if (typeof message !== 'string') {
    errors.push('Message must be a string');
  } else if (message.length > MAX_MESSAGE_LENGTH) {
    errors.push(`Message must not exceed ${MAX_MESSAGE_LENGTH} characters (got ${message.length})`);
  }
  return { valid: errors.length === 0, errors };
}

/**
 * Validate a Stellar asset code.
 * @param code - Asset code string (e.g. "XLM", "USDC").
 */
export function validateAssetCode(code: string): ValidationResult {
  const errors: string[] = [];
  if (!code || typeof code !== 'string') {
    errors.push('Asset code is required');
  } else if (!ASSET_CODE_RE.test(code)) {
    errors.push('Asset code must be 1–12 alphanumeric characters');
  }
  return { valid: errors.length === 0, errors };
}

export interface TipFormValues {
  creator: string;
  amount: bigint;
  tipper: string;
  message?: string;
}

/**
 * Validate all fields of a tip form submission.
 * Returns a combined ValidationResult.
 *
 * @example
 * const result = validateTipForm({ creator: 'G...', amount: 10_000_000n, tipper: 'G...' });
 * if (!result.valid) console.error(result.errors);
 */
export function validateTipForm(values: TipFormValues): ValidationResult {
  const errors: string[] = [];

  const creatorResult = validateAddress(values.creator);
  if (!creatorResult.valid) errors.push(...creatorResult.errors.map((e) => `creator: ${e}`));

  const tipperResult = validateAddress(values.tipper);
  if (!tipperResult.valid) errors.push(...tipperResult.errors.map((e) => `tipper: ${e}`));

  const amountResult = validateTipAmount(values.amount);
  if (!amountResult.valid) errors.push(...amountResult.errors.map((e) => `amount: ${e}`));

  if (values.message !== undefined) {
    const msgResult = validateMessage(values.message);
    if (!msgResult.valid) errors.push(...msgResult.errors.map((e) => `message: ${e}`));
  }

  return { valid: errors.length === 0, errors };
}

export interface BatchTipEntry {
  creator: string;
  amount: bigint;
}

/**
 * Validate a batch tip array.
 * Checks batch size limit and validates each entry.
 */
export function validateBatchTip(entries: BatchTipEntry[]): ValidationResult {
  const errors: string[] = [];

  if (!Array.isArray(entries) || entries.length === 0) {
    errors.push('Batch must contain at least one entry');
    return { valid: false, errors };
  }

  if (entries.length > MAX_BATCH_SIZE) {
    errors.push(`Batch size must not exceed ${MAX_BATCH_SIZE} (got ${entries.length})`);
  }

  entries.forEach((entry, i) => {
    const creatorResult = validateAddress(entry.creator);
    if (!creatorResult.valid) {
      errors.push(...creatorResult.errors.map((e) => `entries[${i}].creator: ${e}`));
    }
    const amountResult = validateTipAmount(entry.amount);
    if (!amountResult.valid) {
      errors.push(...amountResult.errors.map((e) => `entries[${i}].amount: ${e}`));
    }
  });

  return { valid: errors.length === 0, errors };
}
