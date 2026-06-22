import config from '../../../deployment/config.json';

export const TESTNET_CONTRACT_ID = config.networks.testnet.active_contract_id ?? '';
export const MAINNET_CONTRACT_ID = config.networks.mainnet.active_contract_id ?? '';

export const CONTRACT_IDS = {
  testnet: TESTNET_CONTRACT_ID,
  mainnet: MAINNET_CONTRACT_ID,
} as const;

export type Network = keyof typeof CONTRACT_IDS;
