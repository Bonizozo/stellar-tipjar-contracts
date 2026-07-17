export class TipJarError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'TipJarError';
  }
}

export class InvalidAmountError extends TipJarError {
  constructor(message = 'Amount must be a positive bigint') {
    super(message);
    this.name = 'InvalidAmountError';
  }
}

export class TransactionFailedError extends TipJarError {
  txHash?: string;
  constructor(message: string, txHash?: string) {
    super(message);
    this.name = 'TransactionFailedError';
    this.txHash = txHash;
  }
}

export class ContractNotInitializedError extends TipJarError {
  constructor(message = 'Contract has not been initialized') {
    super(message);
    this.name = 'ContractNotInitializedError';
  }
}

export class NetworkError extends TipJarError {
  retries: number;
  constructor(message: string, retries: number) {
    super(message);
    this.name = 'NetworkError';
    this.retries = retries;
  }
}
