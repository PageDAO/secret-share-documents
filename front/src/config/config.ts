import { IToken, NetworkEnum } from "./types";

export const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";
export const MATIC_ADDRESS = "0x0000000000000000000000000000000000001010";

export type Config = {
  networkId: NetworkEnum;
  // subgraphUrl: string;
  contracts: { 
    polygonToSecret: `0x${string}`,
    secretKeyStoreContract: string,
  };
  secretKeyStoreHash: string,
  tokens: { [key: string]: IToken };
};

export const maxDecimals = {
  ETH: 2,
};

export const FEE_RATE_DIVIDER = 10_000;

const mainnet: Config = {
  networkId: NetworkEnum.MAINNET,
  contracts: {
    polygonToSecret: '0x5F3BA1c07b1D550b27Dd2653D2B77d3B673Dd252',
    secretKeyStoreContract: "secret1a6f0kln5xxj8p2pwfpkh94er39rn8zg0xlmwd8",
  },
  secretKeyStoreHash: '13dbf50722df90697d7c8eb15c76bcac56281839a224e946d34c91b528150a3e',
  tokens: {
    [MATIC_ADDRESS]: {
      address: MATIC_ADDRESS,
      symbol: "MATIC",
      name: "MATIC",
      decimals: 18,
    },
  },
};

const chains: { [networkId in NetworkEnum]: Config } = {
  [NetworkEnum.MAINNET]: mainnet,
};

export const getConfig = (networkId: NetworkEnum) => chains[networkId];
