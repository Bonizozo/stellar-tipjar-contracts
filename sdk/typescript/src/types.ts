export type Network = 'testnet' | 'mainnet';

export interface SdkConfig {
  contractId: string;
  network: Network;
  /** Override the default RPC URL for the chosen network. */
  rpcUrl?: string;
}

export interface TipParams {
  creator: string;
  amount: bigint;
  tipper: string;
  memo?: string;
}

export interface TipResult {
  txHash: string;
  creator: string;
  amount: bigint;
}

export interface WithdrawResult {
  txHash: string;
  creator: string;
  amount: bigint;
}

export interface TipEvent {
  sender: string;
  amount: bigint;
}

export interface WithdrawEvent {
  amount: bigint;
}

/** Creator profile with optional category and tags. */
export interface CreatorProfile {
  address: string;
  username?: string;
  categoryId?: string;
  tags?: string[];
  totalTips?: bigint;
  withdrawableBalance?: bigint;
}

// ── Streaming Protocol Types ────────────────────────────────────────────────

export interface StreamParams {
  sender: string;
  creator: string;
  token: string;
  amountPerSecond: bigint;
  duration: bigint;
}

export interface StreamResult {
  txHash: string;
  streamId: bigint;
  sender: string;
  creator: string;
  token: string;
  amountPerSecond: bigint;
  duration: bigint;
}

export interface StreamWithdrawResult {
  txHash: string;
  streamId: bigint;
  creator: string;
  amount: bigint;
}

export interface StreamControlResult {
  txHash: string;
  streamId: bigint;
}

export enum StreamStatus {
  Active = 'Active',
  Paused = 'Paused',
  Cancelled = 'Cancelled',
  Completed = 'Completed',
}

export interface Stream {
  streamId: bigint;
  sender: string;
  creator: string;
  token: string;
  amountPerSecond: bigint;
  startTime: bigint;
  endTime: bigint;
  withdrawn: bigint;
  status: StreamStatus;
  createdAt: bigint;
  updatedAt: bigint;
}

export interface StreamEvent {
  streamId: bigint;
  sender: string;
  creator: string;
  amountPerSecond: bigint;
  amount: bigint;
  token: string;
}

