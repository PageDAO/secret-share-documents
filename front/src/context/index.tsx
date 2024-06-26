"use client";

import React, { ReactNode } from "react";
import { config } from "@/config";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { State, WagmiProvider } from "wagmi";
import { SecretDocumentProvider } from "./SecretDocumentProvider";
import Environment from "../../../sdk-js/src/Environment";
import Config from "../../../sdk-js/src/Config";
import IpfsStorage from "../../../sdk-js/src/StoreDocument/Storage/IPFSStorage";
import PinataStorage from "../../../sdk-js/src/StoreDocument/Storage/PinataStorage";

const queryClient = new QueryClient();

// Setup SDK configuration
const configSecretDocument = new Config({ env: Environment.MAINNET });

const authorizationToken = Buffer.from(`${process.env.NEXT_PUBLIC_INFURA_ID}:${process.env.NEXT_PUBLIC_INFURA_SECRET}`).toString("base64");
const authorization = `Basic ${authorizationToken}`;

const ipfsStorage = new IpfsStorage({
  gateway: "https://ipfs.infura.io",
  port: 5001,
  config: {
    headers: {
      authorization,
    },
  },
});

const pinataStorage = new PinataStorage(
  process.env.NEXT_PUBLIC_PINATA_GATEWAY || "",
  process.env.NEXT_PUBLIC_PINATA_STORAGE || "",
)

configSecretDocument.useStorage(pinataStorage);

export function ContextProvider({
  children,
  initialState,
}: {
  children: ReactNode;
  initialState?: State;
}) {
  console.log("configSecretDocument", configSecretDocument);

  return (
    <WagmiProvider config={config} initialState={initialState}>
      <QueryClientProvider client={queryClient}>
        <SecretDocumentProvider config={configSecretDocument}>
          {children}
        </SecretDocumentProvider>
      </QueryClientProvider>
    </WagmiProvider>
  );
}
