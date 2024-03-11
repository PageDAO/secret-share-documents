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
  const CurrentWallet = useWalletClient();

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
    wallet: CurrentWallet,
  });

  useEffect(() => {
    const fetchIds = async () => {
        const allIds = await secretDocument.findAll();
        setIds(allIds);
    };

    fetchIds();
    }, []);

    const handleIdClick = async (fileId: string) => {
        const document = await secretDocument.getFile(fileId);
        setSelectedDocument(document);
    };

  return (
    <div>
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
  );
}