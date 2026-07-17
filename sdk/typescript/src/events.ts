/** Event parsing utilities for TipJar on-chain events. */
import { SorobanRpc, scValToNative } from '@stellar/stellar-sdk';
import { TipEvent, WithdrawEvent } from './types';

/**
 * Parse a raw Soroban event into a typed TipEvent.
 * Expects event value to be `(sender: Address, amount: i128)`.
 */
export function parseTipEvent(event: SorobanRpc.Api.EventResponse): TipEvent {
  const [senderVal, amountVal] = event.value as unknown[];
  return {
    sender: scValToNative(senderVal as Parameters<typeof scValToNative>[0]) as string,
    amount: BigInt(scValToNative(amountVal as Parameters<typeof scValToNative>[0]) as string),
  };
}

/**
 * Parse a raw Soroban event into a typed WithdrawEvent.
 * Expects event value to be `(amount: i128)`.
 */
export function parseWithdrawEvent(event: SorobanRpc.Api.EventResponse): WithdrawEvent {
  const [amountVal] = event.value as unknown[];
  return {
    amount: BigInt(scValToNative(amountVal as Parameters<typeof scValToNative>[0]) as string),
  };
}
