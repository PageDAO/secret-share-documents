// define ipfsstorage class using the istorage interface (similar to the fakestorage implementation)
import IStorage from "./IStorage";
import IEncryptedData from "../Encryption/IEncryptedData";
import IUploadOptions from "./IUploadOptions";
import axios from 'axios';

const ipfsFileApiUrl = 'https://ipfs.nftbookbazaar.com/api/v0/add';
class IPFSStorage implements IStorage {
  async upload(
    encryptedData: IEncryptedData,
    options: IUploadOptions,
  ): Promise<any> {
      this.pinFileToIPFS(encryptedData.data, 'filename.pdf')
  }

  public pinFileToIPFS = async (file, filename) => {
    try {
      let data = new FormData();
      // convert file to a blob
      const blob = new Blob([file]);
      data.append('file', blob, filename)
      const res = await axios.post(ipfsFileApiUrl,
        data,
        {
          headers: {
            'Content-Type': `multipart/form-data;` // boundary= ${data._boundary}`
          }
        }
      );
      console.log(res.data);
    } catch (error) {
      console.log(error);
    }
  }
}

export default IPFSStorage;