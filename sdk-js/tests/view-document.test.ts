import { expect, test, describe } from "@jest/globals";
import SecretDocumentSmartContract from "../src/SmartContract/SecretDocumentSmartContract";
import { SecretNetworkClient, Wallet } from "secretjs";
import ViewDocument from "../src/ViewDocument";
import { store } from "./utils";
import ViemClient from "../src/SmartContract/ViemClient";
import StoreDocument from "../src/StoreDocument";
import PolygonToSecretSmartContract from "../src/SmartContract/PolygonToSecretSmartContract";
import FakeStorage from "../src/StoreDocument/Storage/FakeStorage";

async function init() {
  const config = globalThis.__SECRET_DOCUMENT_CONFIG__;

  const wallet = new Wallet(process.env.SECRET_NETWORK_WALLET_MNEMONIC);

  const secretNetworkClient = new SecretNetworkClient({
    url: config.getSecretNetwork().endpoint,
    chainId: config.getSecretNetwork().chainId,
    wallet: wallet,
    walletAddress: wallet.address,
  });

  const secretDocument = new SecretDocumentSmartContract({
    chainId: config.getSecretNetwork().chainId,
    client: secretNetworkClient,
    contract: config.getShareDocument(),
    wallet: wallet,
  });

  const viewDocument = new ViewDocument({
    secretDocument: secretDocument,
  });

  const viemClient = new ViemClient({
    chain: config.getChain(config.getChainId()),
    walletConfig: config.getEvmWallet(),
    contract: config.getPolygonToSecret(),
  });

  const polygonToSecret = new PolygonToSecretSmartContract({
    secretContract: config.getShareDocument(),
    viemClient: viemClient,
  });

  const storeDocument = new StoreDocument({
    storage: new FakeStorage(),
    secretDocument: secretDocument,
    polygonToSecret: polygonToSecret,
  });

  return {
    storeDocument,
    secretDocument,
    viewDocument,
  };
}

test("Get all files the user is allowed acces to", async () => {
  const { viewDocument, secretDocument, storeDocument } = await init();

  await store({
    secretDocument: secretDocument,
    storeDocument: storeDocument,
    fileUrl: "https://school.truchot.co/ressources/brief-arolles-bis.pdf",
  });

  const data = await viewDocument.getAllFileIds();

  expect(data).toBeDefined();
  expect(data.length).toBeGreaterThanOrEqual(1);
}, 100_000);

test("Find file content from fileId", async () => {
  const { viewDocument, secretDocument, storeDocument } = await init();

  await store({
    secretDocument: secretDocument,
    storeDocument: storeDocument,
    fileUrl: "https://school.truchot.co/ressources/brief-arolles-bis.pdf",
  });

  const allData = await viewDocument.getAllFileIds();
  const data = await viewDocument.download(allData[0]);

  console.log(data);

  expect(data).toBeDefined();
  expect(data).toHaveProperty("url");
}, 100_000);
