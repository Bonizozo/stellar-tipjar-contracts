export class TipJarError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'TipJarError';
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class NetworkError extends TipJarError {
  constructor(message: string) {
    super(message);
    this.name = 'NetworkError';
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class TransactionError extends TipJarError {
  constructor(message: string) {
    super(message);
    this.name = 'TransactionError';
    Object.setPrototypeOf(this, new.target.prototype);
  }
}
