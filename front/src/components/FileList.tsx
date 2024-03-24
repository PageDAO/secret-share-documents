import React, { useState, useEffect } from 'react';
import Config from "../sdk-js/src/Config";


import ECDHEncryption from "../sdk-js/src/StoreDocument/Encryption/ECDHEncryption";
import SecretDocumentSmartContract from "../sdk-js/src/SmartContract/SecretDocumentSmartContract";
import { SecretNetworkClient, Wallet } from "secretjs";
import { useAccount, useWalletClient } from "wagmi";


export default function FileList() {
  const [decryptedData, setDecryptedData] = useState("");
  const [ids, setIds] = useState<string[]>([]);
  const [selectedDocument, setSelectedDocument] = useState(null);
  const config = new Config();

  const wallet = new Wallet(process.env.SECRET_NETWORK_WALLET_MNEMONIC);
  console.log('wallet',wallet);

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

  useEffect(() => {
    const fetchIds = async () => {
        const allIds = await secretDocument.findAll();
        setIds(allIds);
        console.log('allIds',allIds);
    };

    fetchIds();
    }, []);

    const handleIdClick = async (fileId: string) => {
        const document = await secretDocument.getFile(fileId);
        setSelectedDocument(document);
    };

  return (
    <div className="relative">
        <div className="absolute inset-x-0 bottom-0 h-1/2 bg-white" />
        <div className="mx-auto py-10 max-w-7xl sm:px-6 lg:px-8">
          <div className="relative shadow-xl sm:overflow-hidden sm:rounded-2xl">
            <h2><span className="block text-indigo-200">
              Saved Files:
            </span>
            </h2>
            <ul>
              {ids.map(id => (
                <li key={id} onClick={() => handleIdClick(id)} style={{cursor: 'pointer'}}>
                  {id}
                </li>
              ))}
            </ul>
            {selectedDocument && (
              <div>
                <pre>{JSON.stringify(selectedDocument, null, 2)}</pre>
              </div>
            )}
          </div>
        </div>
    </div>
  );
}