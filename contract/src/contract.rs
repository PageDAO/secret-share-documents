
use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};

use secp256k1::ecdh::SharedSecret;
use secp256k1::{PublicKey, Secp256k1, SecretKey};

use cosmwasm_storage::ReadonlyPrefixedStorage;
use secret_toolkit::serialization::{Json, Serde};


use crate::error::{ContractError, CryptoError};
use crate::msg::{ContractKeyResponse, ExecuteMsg, FileIdsResponse, FilePayloadResponse, InstantiateMsg, QueryMsg};

use crate::state::{
    may_load, save, ContractKeys, FileState, UserInfo, CONTRACT_KEYS, PREFIX_FILES, PREFIX_USERS
};

use cosmwasm_storage::PrefixedStorage;

use sha2::{Digest, Sha256};

use aes_siv::aead::generic_array::GenericArray;
use aes_siv::siv::Aes128Siv;

use hex;

// use ethabi::{decode, ParamType};


#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, StdError> {
    // Create the public/private keys for the contract
    let rng = env.block.random.unwrap().0;
    let secp = Secp256k1::new();

    let private_key = SecretKey::from_slice(&rng).unwrap();
    let private_key_string = private_key.to_string();
    let private_key_bytes = hex::decode(private_key_string).unwrap();

    let public_key = PublicKey::from_secret_key(&secp, &private_key);
    let public_key_bytes = public_key.serialize().to_vec();

    let my_keys = ContractKeys {
        private_key: private_key_bytes,
        public_key: public_key_bytes,
    };

    CONTRACT_KEYS.save(deps.storage, &my_keys)?;


    deps.api
        .debug(&format!("Contract was initialized by {}", info.sender));

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    // TODO :: Prefix function by `try_`
    match msg {
        ExecuteMsg::StoreNewFile { owner, payload } => request_store_new_file(deps, owner, payload),
        ExecuteMsg::EncryptedFilePayload {
            payload,
            public_key: user_public_key,
        } => decrypt_and_store(deps, payload, user_public_key),
        ExecuteMsg::ReceiveMessageEvm {
            source_chain,
            source_address,
            payload,
        } => receive_message_evm(deps, source_chain, source_address, payload),
        _ => Ok(Response::default())
    }
}

pub fn receive_message_evm(
    deps: DepsMut,
    _source_chain: String,
    _source_address: String,
    payload: Binary,
) -> Result<Response, ContractError> {

    // decode the payload
    // executeMsgPayload: [sender, message]
    // let decoded = decode(&vec![ParamType::Bytes], payload.as_slice()).unwrap();

    let data = Json::deserialize::<ExecuteMsg>(&payload.as_slice()).map(Some);

    let execute_msg = match data {
        Ok(Some(d)) => d,
        _ => return Err(ContractError::CustomError {
            val: format!("Invalid execute message"),
        }),
        // Err(error) => panic!("Error when loading file from storage: {:?}", error),   
    };

    match execute_msg {
        ExecuteMsg::EncryptedFilePayload {
            payload,
            public_key: user_public_key,
        } => decrypt_and_store(deps, payload, user_public_key),
        _ => Err(ContractError::CustomError {
            val: format!("Cannot handle other message type"),
        }),
    }

}



/// Decrypt a cyphertext using a given public key and the contract private key.
///
/// Create a shared secret by using the user public key and the contract private key.
/// Then, used this shared secet to decrypt the cyphertext.
fn _decrypt_with_user_public_key(
    deps: &DepsMut,
    payload: Binary,
    user_public_key: Vec<u8>,
) -> Result<ExecuteMsg, ContractError> {
    // Read the private key from the storage
    let contract_keys = CONTRACT_KEYS.load(deps.storage)?;

    let contract_private_key = SecretKey::from_slice(contract_keys.private_key.as_slice())
        .map_err(|e| ContractError::CustomError {
            val: format!("Invalid private key: {}", e),
        })?;

    let other_public_key = PublicKey::from_slice(user_public_key.as_slice()).map_err(|e| {
        ContractError::CustomError {
            val: format!("Invalid public key: {}", e),
        }
    })?;

    // Create a shared secret from the user public key and the conrtact private key
    let shared_secret = SharedSecret::new(&other_public_key, &contract_private_key);
    let key = shared_secret.to_vec();

    let ad_data: &[&[u8]] = &[];
    let ad = Some(ad_data);

    match aes_siv_decrypt(&payload, ad, &key) {
        Ok(decrypted_data) => {

            // Cannot use Bincode2 as float issue
            // Need to change to other way to decode it
            // let data = Bincode2::deserialize::<ExecuteMsg>(&decrypted_data).map(Some);

            // TODO :: See if I can map to a ExecuteMsg directly instead of a Some
            let data = Json::deserialize::<ExecuteMsg>(&decrypted_data).map(Some);

            // println!("Here the data deserialized: {:?}", data);

            match data {
                Ok(d) => match d {
                    Some(msg) => Ok(msg),
                    None => Err(ContractError::CustomError {
                        val: format!("Error empty object when deserialized"),
                    }),
                },
                Err(e) => Err(ContractError::CustomError {
                    val: format!("Error when deserialize payload message {:?}", e.to_string()),
                })
            }
        }
        Err(_e) => {
            // warn!("Error decrypting data: {:?}", e);
            println!("Hola some issue here Crypto error.");
            // Optionally, return an error here if you need to indicate a failure to the caller
            Err(ContractError::CustomError {
                val: format!("Invalid public key"),
            })
        }
    }
}

pub fn decrypt_and_store(
    deps: DepsMut,
    payload: Binary,
    user_public_key: Vec<u8>,
) -> Result<Response, ContractError> {
    match _decrypt_with_user_public_key(&deps, payload, user_public_key) {
        Ok(ExecuteMsg::StoreNewFile {
            owner,
            payload,
        }) => {
            println!("Alright");
            let _ = request_store_new_file(deps, owner, payload);
        }
        Ok(_) => {
            println!("Other message");
        }
        Err(_e) => {
            println!("Error");
        }
    };

    Ok(Response::default())
}

pub fn aes_siv_decrypt(
    plaintext: &[u8],
    ad: Option<&[&[u8]]>,
    key: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let ad = ad.unwrap_or(&[&[]]);

    let mut cipher = Aes128Siv::new(GenericArray::clone_from_slice(key));
    cipher.decrypt(ad, plaintext).map_err(|_e| {
        // warn!("aes_siv_encrypt error: {:?}", e);
        CryptoError::EncryptionError
    })
}

pub fn request_store_new_file(
    deps: DepsMut,
    owner: Addr,
    payload: String,
) -> Result<Response, ContractError> {
    // Store the payload data
    match store_new_file(deps, owner, payload) {
        Ok(_key) => Ok(Response::default()),
        Err(_data) => Ok(Response::default()),
    }
}

/// Create a key from a given file data.
///
/// When storing new data, we need to have a unique key for the given data.
/// We decide to create a hash using the struct element to have a unique
/// key, allowing us to store the data.
///
pub fn create_key_from_file_state(file_state: &FileState) -> [u8; 32] {
    // Encode the file state struct data
    let encoded_file_state: Vec<u8> = bincode::serialize(file_state).unwrap();

    // Hash the data
    let mut hasher = Sha256::new();
    hasher.update(&encoded_file_state);

    // Retrieve the hash
    let key: [u8; 32] = hasher.finalize().into();

    key
}


/// Add a key to a user
pub fn store_new_key(deps: DepsMut, owner: Addr, file_key: [u8; 32]) -> StdResult<()> {

    let user_address = owner.as_bytes();

    // Get user storage
    let mut users_store = PrefixedStorage::new(deps.storage, PREFIX_USERS);
    let loaded_info: StdResult<Option<UserInfo>> = may_load(&users_store, user_address);

    let mut user_info = match loaded_info {
        Ok(Some(user_info)) => user_info,
        Ok(None) => UserInfo {
            files: Vec::new()
        },
        Err(error) => {
            println!("Error when retrieving the data, TODO :: have exception here : {:?}", error);
            return Ok(());
        }
    };

    user_info.files.push(file_key);

    save(&mut users_store, user_address, &user_info)
}


/// Store a new file in the smartcontract storage
///
pub fn store_new_file(deps: DepsMut, owner: Addr, payload: String) -> StdResult<String> {
    // Get the storage for files
    let mut file_storage = PrefixedStorage::new(deps.storage, PREFIX_FILES);

    // Create the file content
    let file_state = FileState {
        owner: owner.clone(),
        payload: payload,
    };

    let key: [u8; 32] = create_key_from_file_state(&file_state);

    // Save the file
    save(&mut file_storage, &key, &file_state)?;

    // Add the key to the user
    // TODO :: handle error
    let _ = store_new_key(deps, owner, key);

    Ok(hex::encode(&key))
}

/// Read the data from the storage
pub fn load_file(deps: Deps, key: String) -> StdResult<String> {
    // TODO :: Future version :: Need to verify the user

    let extracted_key = match hex::decode(key) {
        Ok(k) => k,
        _ => panic!("Invalid key"),
    };

    let files_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_FILES);
    let loaded_payload: StdResult<Option<FileState>> = may_load(&files_store, &extracted_key);

    let payload: String = match loaded_payload {
        Ok(Some(file_state)) => file_state.payload,
        Ok(None) => panic!("Error."),
        Err(_error) => panic!("Error."),
    };

    Ok(payload)
}

// pub fn try_reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
//     config(deps.storage).update(|mut state| -> Result<_, ContractError> {
//         if info.sender != state.owner {
//             return Err(ContractError::Unauthorized {});
//         }
//         state.count = count;
//         Ok(state)
//     })?;

//     deps.api.debug("count reset successfully");
//     Ok(Response::default())
// }

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetFileContent { key } => to_binary(&query_file_content(deps, key)?),
        QueryMsg::GetContractKey {} => to_binary(&query_key(deps)?),
        QueryMsg::GetFileIds {} => to_binary(&query_file_ids(deps, env)?),
    }
}

fn query_file_ids(deps: Deps, env: Env) -> StdResult<FileIdsResponse> {

    let sender = env.contract.address.as_bytes();

    // Get user storage
    let users_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_USERS);
    let loaded_payload: StdResult<Option<UserInfo>> = may_load(&users_store, sender);

    match loaded_payload {
        Ok(Some(user_info)) => Ok(FileIdsResponse { ids: user_info.files }),
        Ok(None) => panic!("File not found from the given key."),
        Err(error) => panic!("Error when loading file from storage: {:?}", error),        
    }

}

fn query_key(deps: Deps) -> StdResult<ContractKeyResponse> {
    let contract_keys = CONTRACT_KEYS.load(deps.storage)?;
    Ok(ContractKeyResponse {
        public_key: contract_keys.public_key,
    })
}

fn query_file_content(deps: Deps, key: String) -> StdResult<FilePayloadResponse> {
    let file_content: String = load_file(deps, key)?;

    Ok(FilePayloadResponse {
        payload: file_content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};
    use secret_toolkit::serialization::{Bincode2, Serde};

    use crate::msg::ExecuteMsg::StoreNewFile;

    use cosmwasm_std::Api;

    use crate::state::may_load;
    use crate::state::PREFIX_FILES;
    use cosmwasm_storage::ReadonlyPrefixedStorage;

    use rstest::fixture;
    use rstest::*;

    // Some references
    // https://github.com/desmos-labs/desmos-contracts/blob/master/contracts/poap/src/contract_tests.rs

    // See this project - organization
    // https://github.com/desmos-labs/desmos-contracts/tree/master/contracts/poap

    fn _contract_initialization() {
        let mut deps = mock_dependencies();

        // Instanciate our Secret Contract
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(0, ""));
        let _response = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[fixture]
    fn contract_public_key() -> Vec<u8> {
        let deps = mock_dependencies();

        _contract_initialization();

        let msg = QueryMsg::GetContractKey {};
        let response = query(deps.as_ref(), mock_env(), msg).unwrap();
        let public_key_response: ContractKeyResponse = from_binary(&response).unwrap();
        public_key_response.public_key
    }

    #[test]
    fn test_store_new_file() {
        let mut deps = mock_dependencies();

        let raw_address = "secretvaloper14c29nyq8e9jgpcpw55e3n7ea4aktxg4xnurynd";
        let owner = deps.api.addr_validate(raw_address).unwrap();
        let payload = String::from("{\"file\": \"content\"}");

        // Store the new file
        let store_new_file_result = store_new_file(deps.as_mut(), owner.clone(), payload.clone());
        let key = match store_new_file_result {
            Ok(storage_key) => storage_key,
            Err(error) => panic!("Error when storing a new file: {:?}", error),
        };

        let expected_key = create_key_from_file_state(&FileState {
            owner: owner.clone(),
            payload: payload.clone(),
        });

        // Verify the key data
        assert_eq!(key, hex::encode(expected_key));

        let extracted_key = match hex::decode(key) {
            Ok(k) => k,
            _ => panic!("Invalid key"),
        };

        assert_eq!(extracted_key, expected_key);

        // read the storage content
        let files_store = ReadonlyPrefixedStorage::new(&deps.storage, PREFIX_FILES);
        let loaded_payload: StdResult<Option<FileState>> = may_load(&files_store, &extracted_key);

        let store_data: FileState = match loaded_payload {
            Ok(Some(file_state)) => file_state,
            Ok(None) => panic!("File not found from the given key."),
            Err(error) => panic!("Error when loading file from storage: {:?}", error),
        };

        assert_eq!(store_data.owner, owner);
        assert_eq!(store_data.payload, payload);
    }

    #[rstest]
    fn test_encrypted_file_payload_request() {
        // contract_initilization();

        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(0, ""));
        let _response = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // let mut deps = mock_dependencies();
        let env = mock_env();

        let raw_address = "secretvaloper14c29nyq8e9jgpcpw55e3n7ea4aktxg4xnurynd";
        let owner = deps.api.addr_validate(raw_address).unwrap();
        let payload = String::from("{\"file\": \"content\"}");

        // Create the message for storing a new file
        let store_new_file_msg = StoreNewFile { 
            owner: owner.clone(), 
            payload: payload.clone() 
        };

        let message = &Json::serialize(&store_new_file_msg).unwrap();

        // Generate public/private key locally
        let rng = env.block.random.unwrap().0;
        let secp = Secp256k1::new();

        let private_key = SecretKey::from_slice(&rng).unwrap();
        let private_key_string = private_key.to_string();
        let private_key_bytes = hex::decode(private_key_string).unwrap();

        let public_key = PublicKey::from_secret_key(&secp, &private_key);
        let public_key_bytes = public_key.serialize().to_vec();

        // Query the contract public key
        let msg = QueryMsg::GetContractKey {};
        let response = query(deps.as_ref(), mock_env(), msg).unwrap();
        let public_key_response: ContractKeyResponse = from_binary(&response).unwrap();
        let contract_public_key = public_key_response.public_key;

        // Create share secret
        let my_private_key = SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| ContractError::CustomError {
                val: format!("Invalid private key: {}", e),
            })
            .unwrap();

        let other_public_key = PublicKey::from_slice(contract_public_key.as_slice())
            .map_err(|e| ContractError::CustomError {
                val: format!("Invalid public key: {}", e),
            })
            .unwrap();

        let shared_secret = SharedSecret::new(&other_public_key, &my_private_key);
        let key = shared_secret.to_vec();

        // Encrypt our payload
        let ad_data: &[&[u8]] = &[];
        let ad = Some(ad_data);
        let ad = ad.unwrap_or(&[&[]]);

        let mut cipher = Aes128Siv::new(GenericArray::clone_from_slice(&key));
        let encrypt_message = cipher
            .encrypt(ad, message)
            .map_err(|_e| CryptoError::EncryptionError)
            .unwrap();

        // Send the request
        let msg = ExecuteMsg::EncryptedFilePayload {
            payload: Binary::from(encrypt_message),
            public_key: public_key_bytes,
        };

        let unauth_env = mock_info("anyone", &coins(0, "token"));
        let _res = execute(deps.as_mut(), mock_env(), unauth_env, msg);
        

        // Check the file has been encrypted
        // TODO :: remove this verification, check directly by requesting the user key
        // read the storage content
        let expected_key = create_key_from_file_state(&FileState {
            owner: owner.clone(),
            payload: payload.clone(),
        });
    
        let files_store = ReadonlyPrefixedStorage::new(&deps.storage, PREFIX_FILES);
        let loaded_payload: StdResult<Option<FileState>> = may_load(&files_store, &expected_key);

        let store_data: FileState = match loaded_payload {
            Ok(Some(file_state)) => file_state,
            Ok(None) => panic!("File not found from the given key."),
            Err(error) => panic!("Error when loading file from storage: {:?}", error),
        };

        assert_eq!(store_data.owner, owner);
        assert_eq!(store_data.payload, payload);

    }

    // TODO :: ~/Project/examples/EVM-encrypt-decrypt/secret_network

    #[test]
    fn keys_initialization() {
        let mut deps = mock_dependencies();

        // Instanciate our Secret Contract
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(0, ""));
        let response = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, response.messages.len());

        // Get the generated public key
        let msg = QueryMsg::GetContractKey {};
        let response = query(deps.as_ref(), mock_env(), msg).unwrap();
        let public_key_response: ContractKeyResponse = from_binary(&response).unwrap();
        let public_key = public_key_response.public_key;
        assert!(public_key.len() == 33); // We have an additional 1 byte prefix for the X-coordinate

        // Verify that the public key is the same as the one we store
        let contract_keys = CONTRACT_KEYS.load(deps.as_mut().storage);
        let storage_public_key = match contract_keys {
            Ok(keys) => keys.public_key,
            Err(error) => panic!("Error when loading key from storage: {:?}", error),
        };
        assert!(public_key == storage_public_key);
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // TODO :: Let's query the publc key of the smart contract
    }
}
