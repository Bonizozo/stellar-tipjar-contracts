export interface SendTipParams {
    creator: string;
    amount: bigint;
    tipper: string;
    memo?: string;
}
export interface TipResult {
    success: boolean;
    txHash: string;
    ledger: number;
}
export interface WithdrawResult {
    success: boolean;
    txHash: string;
    ledger: number;
}
//# sourceMappingURL=types.d.ts.map