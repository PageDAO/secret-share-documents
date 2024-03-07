import { parseEther } from "viem";
import ISecretNetworkSmartContract from "./ISecretNetworkSmartContract";
import ViemClient from "./ViemClient";
import {
  AxelarQueryAPI,
  AxelarQueryAPIFeeResponse,
  Environment,
  EvmChain,
  GasToken,
} from "@axelar-network/axelarjs-sdk";

import { WalletClient } from 'viem';

interface Props {
  secretContract: ISecretNetworkSmartContract;
  walletClient: WalletClient;
}

class PolygonToSecretSmartContract {
  secretContract: ISecretNetworkSmartContract;
  polygonToSecret: IPolygonSmartContract;
  walletClient: WalletClient;

  constructor({ secretContract, polygonToSecret, walletClient }: Props) {
    this.walletClient = walletClient;
    this.secretContract = secretContract;
    this.polygonToSecret = polygonToSecret;
  }

  async getEstimateFee(): Promise<AxelarQueryAPIFeeResponse> {
    const axelar = new AxelarQueryAPI({
      environment: Environment.MAINNET,
    });

    const gmpParams = {
      showDetailedFees: true,
      destinationContractAddress: this.secretContract.address,
      sourceContractAddress: this.viemClient.getContract().address,
      tokenSymbol: GasToken.MATIC,
    };

    return (await axelar.estimateGasFee(
      EvmChain.POLYGON,
      "secret-snip",
      GasToken.MATIC,
      BigInt(100000),
      "auto",
      "0",
      gmpParams,
    )) as AxelarQueryAPIFeeResponse;
  }

  async send(message: any): Promise<`0x${string}`> {
    const gasEstimate = await this.getEstimateFee();

    return await this.viemClient.writeContract({
      functionName: "send",
      args: [
        "secret-snip", 
        this.secretContract.address, 
        message
      ],
      value: this.viemClient.formatEther(BigInt(gasEstimate.executionFee)),
    });
  }
}

export default PolygonToSecretSmartContract;
