"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.TransactionError = exports.NetworkError = exports.TipJarError = void 0;
class TipJarError extends Error {
    constructor(message) {
        super(message);
        this.name = 'TipJarError';
        Object.setPrototypeOf(this, new.target.prototype);
    }
}
exports.TipJarError = TipJarError;
class NetworkError extends TipJarError {
    constructor(message) {
        super(message);
        this.name = 'NetworkError';
        Object.setPrototypeOf(this, new.target.prototype);
    }
}
exports.NetworkError = NetworkError;
class TransactionError extends TipJarError {
    constructor(message) {
        super(message);
        this.name = 'TransactionError';
        Object.setPrototypeOf(this, new.target.prototype);
    }
}
exports.TransactionError = TransactionError;
