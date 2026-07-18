import {
  validateAddress,
  validateTipAmount,
  validateMessage,
  validateAssetCode,
  validateTipForm,
  validateBatchTip,
  MAX_MESSAGE_LENGTH,
  MAX_BATCH_SIZE,
} from '../validation';

describe('validateAddress', () => {
  it('accepts a valid G-address', () => {
    const result = validateAddress('GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNA');
    expect(result.valid).toBe(true);
  });

  it('accepts a valid C-address (contract)', () => {
    const result = validateAddress('CAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNA');
    expect(result.valid).toBe(true);
  });

  it('rejects an empty string', () => {
    const result = validateAddress('');
    expect(result.valid).toBe(false);
    expect(result.errors.length).toBeGreaterThan(0);
  });

  it('rejects a short address', () => {
    const result = validateAddress('GABC123');
    expect(result.valid).toBe(false);
  });
});

describe('validateTipAmount', () => {
  it('accepts a positive amount', () => {
    expect(validateTipAmount(1n).valid).toBe(true);
    expect(validateTipAmount(10_000_000n).valid).toBe(true);
  });

  it('rejects zero', () => {
    const result = validateTipAmount(0n);
    expect(result.valid).toBe(false);
  });

  it('rejects negative amounts', () => {
    const result = validateTipAmount(-1n);
    expect(result.valid).toBe(false);
  });
});

describe('validateMessage', () => {
  it('accepts an empty message', () => {
    expect(validateMessage('').valid).toBe(true);
  });

  it('accepts a message at the limit', () => {
    expect(validateMessage('a'.repeat(MAX_MESSAGE_LENGTH)).valid).toBe(true);
  });

  it('rejects a message over the limit', () => {
    const result = validateMessage('a'.repeat(MAX_MESSAGE_LENGTH + 1));
    expect(result.valid).toBe(false);
    expect(result.errors[0]).toContain('280');
  });
});

describe('validateAssetCode', () => {
  it('accepts XLM', () => {
    expect(validateAssetCode('XLM').valid).toBe(true);
  });

  it('accepts USDC', () => {
    expect(validateAssetCode('USDC').valid).toBe(true);
  });

  it('rejects empty string', () => {
    expect(validateAssetCode('').valid).toBe(false);
  });

  it('rejects code longer than 12 chars', () => {
    expect(validateAssetCode('TOOLONGASSET1').valid).toBe(false);
  });
});

describe('validateTipForm', () => {
  const validCreator = 'GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNA';
  const validTipper = 'GBVVJJWBKQZFKQZFKQZFKQZFKQZFKQZFKQZFKQZFKQZFKQZFKQZFKQZA';

  it('passes with valid inputs', () => {
    const result = validateTipForm({
      creator: validCreator,
      amount: 10_000_000n,
      tipper: validTipper,
    });
    expect(result.valid).toBe(true);
  });

  it('fails with zero amount', () => {
    const result = validateTipForm({ creator: validCreator, amount: 0n, tipper: validTipper });
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes('amount'))).toBe(true);
  });

  it('fails with invalid creator', () => {
    const result = validateTipForm({ creator: 'bad', amount: 1n, tipper: validTipper });
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes('creator'))).toBe(true);
  });

  it('fails with message too long', () => {
    const result = validateTipForm({
      creator: validCreator,
      amount: 1n,
      tipper: validTipper,
      message: 'x'.repeat(281),
    });
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes('message'))).toBe(true);
  });
});

describe('validateBatchTip', () => {
  const validCreator = 'GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNA';

  it('accepts a valid batch', () => {
    const result = validateBatchTip([{ creator: validCreator, amount: 1n }]);
    expect(result.valid).toBe(true);
  });

  it('rejects empty batch', () => {
    expect(validateBatchTip([]).valid).toBe(false);
  });

  it('rejects batch over max size', () => {
    const entries = Array.from({ length: MAX_BATCH_SIZE + 1 }, () => ({
      creator: validCreator,
      amount: 1n,
    }));
    const result = validateBatchTip(entries);
    expect(result.valid).toBe(false);
    expect(result.errors[0]).toContain('50');
  });
});
