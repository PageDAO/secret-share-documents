
use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};


use secp256k1::ecdh::SharedSecret;
use secp256k1::{PublicKey, Secp256k1, SecretKey};

use cosmwasm_storage::ReadonlyPrefixedStorage;

use secret_toolkit::permit::Permit;
use secret_toolkit::serialization::{Json, Serde};


use crate::error::ContractError;
use crate::msg::{ContractKeyResponse, EncryptedExecuteMsg, ExecuteMsg, ExecuteMsgAction, ExecutePermitMsg, FileIdsResponse, FilePayloadResponse, InstantiateMsg, QueryMsg, QueryWithPermit};

use crate::state::{
    load, may_load, save, Config, ContractKeys, FileState, UserInfo, CONFIG, CONTRACT_KEYS, FILE_PERMISSIONS, PREFIX_FILES, PREFIX_REVOKED_PERMITS, PREFIX_USERS
};

use cosmwasm_storage::PrefixedStorage;

use sha2::{Digest, Sha256};

use aes_siv::aead::generic_array::GenericArray;
use aes_siv::siv::Aes128Siv;

use hex;

// use ethabi::{decode, ParamType};


// TODO :: See Revoking permits - need implementation ?
// I do not think this can be done on a permits execute message
// Maybe need to execute directly on secret network
// https://scrt.university/pathways/33/implementing-viewing-keys-and-permits


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
    let private_key_string = private_key.display_secret().to_string();
    let private_key_bytes = hex::decode(private_key_string).unwrap();

    let public_key = PublicKey::from_secret_key(&secp, &private_key);
    let public_key_bytes = public_key.serialize().to_vec();

    let my_keys = ContractKeys {
        private_key: private_key_bytes,
        public_key: public_key_bytes,
    };

    CONTRACT_KEYS.save(deps.storage, &my_keys)?;

    // Save the configuration of this contract
    let _ = CONFIG.save(deps.storage, &Config {
        contract_address: env.contract.address,
    });


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
    match msg {
        ExecuteMsg::ReceiveMessageEvm {
            source_chain,
            source_address,
            payload,
        } => receive_message_evm(deps, source_chain, source_address, payload)
    }
}


/// Decrypt and execute the message passed from EVM
pub fn receive_message_evm(
    deps: DepsMut,
    _source_chain: String,
    _source_address: String,
    execute_msg: EncryptedExecuteMsg,
) -> Result<Response, ContractError> {

    let user_public_key = execute_msg.public_key;
    let encrypted_data = execute_msg.payload;

    // Decrypt the EVM message
    let decrypt_msg = _decrypt_with_user_public_key(&deps, encrypted_data, user_public_key)?;
    match decrypt_msg {
        ExecutePermitMsg::WithPermit { permit, execute } => {
            return permit_execute_message(deps, permit, execute);
        }
    };
}

/// Verify the permit and check if it is the right users.
/// Returns: verified user address.
fn _verify_permit(deps: Deps, permit: Permit, contract_address: Addr) -> Result<Addr, ContractError> {

    // Get and validate user address
    let account = secret_toolkit::permit::validate(
        deps,
        PREFIX_REVOKED_PERMITS,
        &permit,
        contract_address.into_string(),
        None,
    )?;

    let account = Addr::unchecked(account);

    return Ok(account);
}


fn permit_execute_message(deps: DepsMut, permit: Permit, query: ExecuteMsgAction) -> Result<Response, ContractError> {

    // Verify the account
    let contract_address = CONFIG.load(deps.storage)?.contract_address;
    let account = _verify_permit(deps.as_ref(), permit, contract_address)?;

    // Execute the message
    match query {
        ExecuteMsgAction::StoreNewFile { payload } => {
            let _ = store_new_file(deps, account, payload);
        },
        ExecuteMsgAction::ManageFileRights { file_id, add_viewing, delete_viewing, change_owner } => {
            // let u8_key: [u8; 32] = key.as_bytes().try_into().unwrap();
            // let whitelisted = FILE_PERMISSIONS.get(deps.storage, &(u8_key, account));
            // TODO 

            // Decode the file key 
            let extracted_key = match hex::decode(file_id) {
                Ok(k) => k,
                _ => panic!("Invalid key"),
            };
            let extracted_key: [u8; 32] = extracted_key.try_into().unwrap();
        
            // Check the file exist for the given key
            let files_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_FILES);
            let loaded_file: Option<FileState> = may_load(&files_store, &extracted_key)?;

            let file_state = match loaded_file {
                Some(file_state) => file_state,
                _ => panic!("error")
            };

            if file_state.owner == account {
                let _ = update_file_access(
                    deps,
                    extracted_key, 
                    account,
                    add_viewing, 
                    delete_viewing, 
                    change_owner
                );

            } else {
                panic!("error");
            }

        }
    };

    Ok(Response::default())
}


/// Decrypt a cyphertext using a given public key and the contract private key.
///
/// Create a shared secret by using the user public key and the contract private key.
/// Then, used this shared secet to decrypt the cyphertext.
/// 
/// Note: for the ExecutePermitMsg, we cannot use Bincode2 as encoder as we are using 
/// enum values, which is not manage by this library.
fn _decrypt_with_user_public_key(
    deps: &DepsMut,
    payload: Binary,
    user_public_key: Vec<u8>,
) -> Result<ExecutePermitMsg, ContractError> {
    // Read the private key from the storage
    let contract_keys = CONTRACT_KEYS.load(deps.storage)?;
    let contract_private_key = SecretKey::from_slice(contract_keys.private_key.as_slice()).unwrap();

    // Conver the user public key
    let user_public_key = PublicKey::from_slice(user_public_key.as_slice())
        .map_err(|e| {
            ContractError::InvalidPublicKey { val: e.to_string() }
        })?;

    // Create a shared secret from the user public key and the conrtact private key
    let shared_secret = SharedSecret::new(&user_public_key, &contract_private_key);
    let key = shared_secret.secret_bytes();

    let ad_data: &[&[u8]] = &[];
    let ad = Some(ad_data);

    // Decrypt the data and deserialized the message
    let decrypted_data = aes_siv_decrypt(&payload, ad, &key)?;
    let data = Json::deserialize::<ExecutePermitMsg>(&decrypted_data).map(Some);
    
    match data {
        Ok(execute_permit_message) => {
            match execute_permit_message {
                Some(msg) => Ok(msg),
                None => Err(ContractError::UnknownExecutePermitMsg)
            }
        },
        Err(_) => Err(ContractError::UnknownExecutePermitMsg)
    }
}

pub fn aes_siv_decrypt(
    plaintext: &[u8],
    ad: Option<&[&[u8]]>,
    key: &[u8],
) -> Result<Vec<u8>, ContractError> {
    let ad = ad.unwrap_or(&[&[]]);

    let mut cipher = Aes128Siv::new(GenericArray::clone_from_slice(key));
    cipher.decrypt(ad, plaintext).map_err(|_e| {
        ContractError::EncryptionError
    })
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
    let loaded_info: Option<UserInfo> = may_load(&users_store, user_address)?;

    // Get user info if exists, else create new one
    let mut user_info = match loaded_info {
        Some(user_info) => user_info,
        None => UserInfo {
            files: Vec::new()
        },
    };

    // Update and save the user info
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

    // Add the viewing right for the user
    FILE_PERMISSIONS.insert(deps.storage, &(key, owner.clone()), &true)?;

    // Add the key to the user
    store_new_key(deps, owner.clone(), key)?;

    // Return the key of the file
    Ok(hex::encode(&key))
}



pub fn update_file_access(
    deps: DepsMut,
    file_key: [u8; 32], 
    account: Addr, 
    add_viewing: Vec<Addr>, 
    delete_viewing: Vec<Addr>, 
    change_owner: Addr) -> StdResult<()> {

    // Add all viewing access
    for add in &add_viewing {

        let already_added = FILE_PERMISSIONS.get(deps.storage, &(file_key, account.clone()));
        if already_added.is_none() || already_added.is_some_and(|x| !x) {
            // Add permission
            let _add = FILE_PERMISSIONS.insert(deps.storage, &(file_key, add.clone()), &true);

            // Add the file in the list of user view
            let users_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_USERS);
            let loaded_payload: StdResult<Option<UserInfo>> = may_load(&users_store, account.as_bytes());

            let mut user_info = match loaded_payload {
                Ok(Some(user_info)) => user_info,
                _ => UserInfo {
                    files: Vec::new()
                }
            };

            user_info.files.push(file_key);

            // Save the updated information
            let mut users_store = PrefixedStorage::new(deps.storage, PREFIX_USERS);
            let _saved_result = save(&mut users_store, account.as_bytes(), &user_info);
        }

    };

    for delete in &delete_viewing {

        // Check if the user has a viewing right
        let already_added = FILE_PERMISSIONS.get(deps.storage, &(file_key, account.clone()));
        if already_added.is_some() {
            // Remove permission
            let _delete = FILE_PERMISSIONS.insert(deps.storage, &(file_key, delete.clone()), &false);

            // Remove the file from the user list
            let users_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_USERS);
            let loaded_payload: StdResult<Option<UserInfo>> = load(&users_store, account.as_bytes());

            let mut user_info = match loaded_payload {
                Ok(Some(user_info)) => user_info,
                _ => UserInfo {
                    files: Vec::new()
                }
            };

            let index = user_info.files.iter().position(|x| *x == file_key).unwrap();
            user_info.files.remove(index);

            // Save the modification
            let mut users_store = PrefixedStorage::new(deps.storage, PREFIX_USERS);
            let _saved_result = save(&mut users_store, account.as_bytes(), &user_info);
        }

    };

    // Check if we need to change the owner
    let mut files_store = PrefixedStorage::new(deps.storage, PREFIX_FILES);
    let loaded_file: StdResult<Option<FileState>> = load(&files_store, &file_key);

    let mut file_state = match loaded_file {
        Ok(Some(file_state)) => file_state,
        _ => panic!("error")
    };

    // Update the owner
    if file_state.owner != change_owner {
        file_state.owner = change_owner;
        save(&mut files_store, &file_key, &file_state)?;
    };

    Ok(())
}


// TODO Given a contract id & owner, see the rights



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


#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {    
    match msg {
        QueryMsg::GetContractKey {} => to_binary(&query_key(deps)?),
        QueryMsg::WithPermit { permit, query } => permit_queries(deps, permit, query)
    }
}

fn query_key(deps: Deps) -> StdResult<ContractKeyResponse> {
    let contract_keys = CONTRACT_KEYS.load(deps.storage)?;
    Ok(ContractKeyResponse {
        public_key: contract_keys.public_key,
    })
}

fn permit_queries(deps: Deps, permit: Permit, query: QueryWithPermit) -> Result<Binary, StdError> {
    
    // Verify the account through the permit
    let contract_address = CONFIG.load(deps.storage)?.contract_address;
    let account = match _verify_permit(deps, permit, contract_address) {
        Ok(account) => account,
        Err(e) => panic!("Error {:?}", e),
    };

    // Permit validated! We can now execute the query.
    match query {
        QueryWithPermit::GetFileIds {} => {
            // Get user file
            to_binary(&query_file_ids(deps, account)?)
        },
        QueryWithPermit::GetFileContent { file_id } => {

            // hex::encode(&bytes_key)
            let key = match hex::decode(&file_id) {
                Ok(key) => key,
                _ => return Err(StdError::NotFound { kind: String::from("Invalid key.") })
            };
            let u8_key: [u8; 32] = key.try_into().unwrap();

            // Check the permission - whitelisted
            // let _ = FILE_PERMISSIONS.insert(deps.storage, &(key, owner.clone()), &true);
            let whitelisted = FILE_PERMISSIONS.get(deps.storage, &(u8_key, account));

            match whitelisted {
                Some(authorized) => {
                    if !authorized {
                        return Err(StdError::generic_err(format!(
                            "Unauthorized access for the given file."
                        )));
                    }
                },
                _ => {
                    return Err(StdError::generic_err(format!(
                        "Unauthorized access for the given file."
                    )));
                }
            };

            // Get the file content
            to_binary(&query_file_content(deps, file_id)?)
        }
    }
}

/// Return the file ids given a user.
/// We need to verify with a permit that only the given account is the one that can retrieve the data.
fn query_file_ids(deps: Deps, account: Addr) -> StdResult<FileIdsResponse> {

    // Get user storage
    let users_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_USERS);
    let loaded_payload: StdResult<Option<UserInfo>> = may_load(&users_store, account.as_bytes());

    match loaded_payload {
        Ok(Some(user_info)) => {

            let key_to_string : Vec<String> = user_info.files.iter().map(|&bytes_key| {
                hex::encode(&bytes_key)
            }).collect();

            Ok(FileIdsResponse { file_ids: key_to_string })
        }
        Ok(None) => panic!("File not found from the given key."),
        Err(error) => panic!("Error when loading file from storage: {:?}", error),        
    }

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
    use secret_toolkit::permit::{PermitParams, PermitSignature, PubKey, TokenPermissions};
    use secret_toolkit::serialization::Serde;

    use cosmwasm_std::Api;

    use crate::state::may_load;
    use crate::state::PREFIX_FILES;
    use cosmwasm_storage::ReadonlyPrefixedStorage;

    // Some references
    // https://github.com/desmos-labs/desmos-contracts/blob/master/contracts/poap/src/contract_tests.rs

    // See this project - organization
    // https://github.com/desmos-labs/desmos-contracts/tree/master/contracts/poap

    /// Instanciate a new smart contract
    fn setup_contract(deps: DepsMut) {
        // Instanciate our Secret Contract
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(0, ""));
        let response = instantiate(deps, mock_env(), info, msg).unwrap();
        assert_eq!(0, response.messages.len());
    }

    /// Generate a valid address and a valid permit
    fn _generate_address_with_valid_permit(deps: DepsMut) -> (Addr, Permit) {

        let token_address = CONFIG.load(deps.storage).unwrap().contract_address;

        let user_address = "secret18mdrja40gfuftt5yx6tgj0fn5lurplezyp894y";
        let permit_name = "default";
        let chain_id = "secretdev-1";
        let pub_key = "AkZqxdKMtPq2w0kGDGwWGejTAed0H7azPMHtrCX0XYZG";
        let signature = "ZXyFMlAy6guMG9Gj05rFvcMi5/JGfClRtJpVTHiDtQY3GtSfBHncY70kmYiTXkKIxSxdnh/kS8oXa+GSX5su6Q==";


        let user_permit = Permit {
            params: PermitParams {
                allowed_tokens: vec![String::from(token_address)],
                permit_name: permit_name.to_string(),
                chain_id: chain_id.to_string(),
                permissions: vec![TokenPermissions::Owner],
            },
            signature: PermitSignature {
                pub_key: PubKey {
                    r#type: "tendermint/PubKeySecp256k1".to_string(),
                    value: Binary::from_base64(pub_key).unwrap(),
                },
                signature: Binary::from_base64(signature).unwrap(),
            }
        };

        let user_address = deps.api.addr_validate(user_address).unwrap();

        return (user_address, user_permit)
    }

    fn generate_user_2(deps: DepsMut) -> (Addr, Permit) {
        // https://permits.scrtlabs.com/

        let token_address = CONFIG.load(deps.storage).unwrap().contract_address;

        let user_address = "secret1ncgrta0phcl5t4707sg0qkn0cd8agr95nytfpy";
        let permit_name = "default";
        let chain_id = "secret-4";
        let pub_key = "A9vpITCF1VTnIi+3x8g+IqNV2LAiFyqt4SlaYD+fk+SH";
        let signature = "11zOqv1CKkXodChCLhMVQ2Hqkp1zj/IqyvgjMX55wLpG95c8iZO9Nmo+DgSBBBZVb7sfuApKBFSxPxueoAHu2Q==";


        let user_permit = Permit {
            params: PermitParams {
                allowed_tokens: vec![String::from(token_address)],
                permit_name: permit_name.to_string(),
                chain_id: chain_id.to_string(),
                permissions: vec![TokenPermissions::Owner],
            },
            signature: PermitSignature {
                pub_key: PubKey {
                    r#type: "tendermint/PubKeySecp256k1".to_string(),
                    value: Binary::from_base64(pub_key).unwrap(),
                },
                signature: Binary::from_base64(signature).unwrap(),
            }
        };

        let user_address = deps.api.addr_validate(user_address).unwrap();

        return (user_address, user_permit)
    }


    fn _query_contract_pubic_key(deps: Deps) -> ContractKeyResponse {
        let query_msg = QueryMsg::GetContractKey {};
        let response = query(deps, mock_env(), query_msg).unwrap();
        let key_response: ContractKeyResponse = from_binary(&response).unwrap();
        key_response
    }

    fn _generate_local_public_private_key(env: Env) -> (Vec<u8>, Vec<u8>) {
        // Generate public/private key locally
        let rng = env.block.random.unwrap().0;
        let secp = Secp256k1::new();

        let private_key = SecretKey::from_slice(&rng).unwrap();
        let private_key_string = private_key.display_secret().to_string();
        let private_key_bytes = hex::decode(private_key_string).unwrap();

        let public_key = PublicKey::from_secret_key(&secp, &private_key);
        let public_key_bytes = public_key.serialize().to_vec();

        return (public_key_bytes, private_key_bytes);
    }

    fn _encrypt_with_share_secret(
        local_private_key: Vec<u8>, 
        contract_public_key: Vec<u8>, 
        message_to_encrypt: &Vec<u8>
    ) -> Vec<u8> {
        let my_private_key = SecretKey::from_slice(&local_private_key)
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
        let key = shared_secret.secret_bytes();

        // Encrypt our payload
        let ad_data: &[&[u8]] = &[];
        let ad = Some(ad_data);
        let ad = ad.unwrap_or(&[&[]]);

        let mut cipher = Aes128Siv::new(GenericArray::clone_from_slice(&key));
        let encrypt_message = cipher
            .encrypt(ad, message_to_encrypt)
            .map_err(|_e| ContractError::EncryptionError)
            .unwrap();

        return encrypt_message;
    }

    #[test]
    fn test_contract_initialization() {
        // Initialize the smart contract
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        // Check that the contract generate a public key
        let key_response = _query_contract_pubic_key(deps.as_ref());
        assert_eq!(33, key_response.public_key.len()); // We have an additional 1 byte prefix for the X-coordinate
    }


    #[test]
    fn keys_initialization() {
        // Initialize the smart contract
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());
        
        let key_response = _query_contract_pubic_key(deps.as_ref());
        let public_key = key_response.public_key; 

        // Verify that the public key is the same as the one we store
        let contract_keys = CONTRACT_KEYS.load(deps.as_mut().storage);
        let storage_public_key = match contract_keys {
            Ok(keys) => keys.public_key,
            Err(error) => panic!("Error when loading key from storage: {:?}", error),
        };
        assert!(public_key == storage_public_key);
    }


    /// Create an execute message given a file and a user permit
    fn _create_evm_message(
        deps: Deps,
        file: &String, 
        permit: &Permit
    ) -> ExecuteMsg {

        // Create the message for storing a new file
        let message = &Json::serialize(
            &ExecutePermitMsg::WithPermit { 
                permit: permit.clone(), 
                execute: ExecuteMsgAction::StoreNewFile { 
                    payload: file.clone() 
                } 
            }
        ).unwrap();

        // Query the contract public key
        let contract_public_key = _query_contract_pubic_key(deps).public_key;

        // Generate public/private key locally
        let (local_public_key, local_private_key) = _generate_local_public_private_key(mock_env());

        // Create share secret
        let encrypted_message = _encrypt_with_share_secret(
            local_private_key, 
            contract_public_key, 
            message
        );

        // Create the request
        ExecuteMsg::ReceiveMessageEvm {
            source_chain: String::from("polygon"),
            source_address: String::from("0x329CdCBBD82c934fe32322b423bD8fBd30b4EEB6"),
            payload: EncryptedExecuteMsg {
                payload: Binary::from(encrypted_message),
                public_key: local_public_key,
            },
        }
    }

    fn _query_user_files(deps: Deps, permit: &Permit) -> Vec<String> {
        let query_msg = QueryMsg::WithPermit { 
            permit: permit.clone(),
            query: QueryWithPermit::GetFileIds {}
        };
        let res = query(deps, mock_env(), query_msg);
        assert!(res.is_ok());
        
        let file_id_response = Json::deserialize::<FileIdsResponse>(&res.unwrap()).map(Some);

        // We should now have one key
        file_id_response.unwrap().unwrap().file_ids
    }

    fn _query_file(deps: Deps, permit: Permit, file_key: &String) -> String {
        let query_msg = QueryMsg::WithPermit { 
            permit: permit,
            query: QueryWithPermit::GetFileContent { file_id: file_key.clone() } 
        };

        let response = query(deps, mock_env(), query_msg).unwrap();
        let file_content: FilePayloadResponse = from_binary(&response).unwrap();
        file_content.payload
    }


    #[test]
    fn test_evm_store_new_file() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        // Generate user information & payload
        let (_owner, user_permit) = _generate_address_with_valid_permit(deps.as_mut());
        let payload = String::from("{\"file\": \"content\"}");

        let evm_message = _create_evm_message(deps.as_ref(), &payload, &user_permit);

        // Send the evm message
        let unauth_env = mock_info("anyone", &coins(0, "token"));
        let res_store_file = execute(deps.as_mut(), mock_env(), unauth_env, evm_message);
        assert!(res_store_file.is_ok());
        
        // Query the user file
        let user_file = _query_user_files(deps.as_ref(), &user_permit);
        
        // We should have the file we previously stored
        assert_eq!(user_file.len(), 1); 

        // Query with the user the file
        let file_content = _query_file(deps.as_ref(), user_permit, &user_file[0]);

        // Verify that the store data is the same as the input one
        assert_eq!(file_content, payload);
    }


    #[test]
    fn test_retrieve_file_from_invalid_key() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        let (_owner, user_permit) = _generate_address_with_valid_permit(deps.as_mut());
        
        // Query with the user the file
        let query_msg = QueryMsg::WithPermit { 
            permit: user_permit, 
            query: QueryWithPermit::GetFileContent { file_id: String::from("invalid_key") } 
        };

        let response = query(deps.as_ref(), mock_env(), query_msg);
        assert!(response.is_err());
    }


    #[test]
    fn test_retrieve_file_with_no_access() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        // Generate user information & payload
        let (_owner, user_permit) = _generate_address_with_valid_permit(deps.as_mut());
        let payload = String::from("{\"file\": \"content\"}");

        let evm_message = _create_evm_message(deps.as_ref(), &payload, &user_permit);

        // Send the evm message
        let unauth_env = mock_info("anyone", &coins(0, "token"));
        let res_store_file = execute(deps.as_mut(), mock_env(), unauth_env, evm_message);
        assert!(res_store_file.is_ok());
        
        // Query the user file
        let user_file = _query_user_files(deps.as_ref(), &user_permit);
        
        // We should have the file we previously stored
        assert_eq!(user_file.len(), 1); 

        // Generate another user
        let (_user_2, user_permit_2) = generate_user_2(deps.as_mut());

        // Try to get the file of the user 1
        let query_msg = QueryMsg::WithPermit { 
            permit: user_permit_2.clone(),
            query: QueryWithPermit::GetFileContent { file_id: user_file[0].clone() } 
        };
        let response = query(deps.as_ref(), mock_env(), query_msg);
        assert!(response.is_err());
    }




    #[test]
    fn test_unauthorized_file_access() {
        
        let user_address = "secret18mdrja40gfuftt5yx6tgj0fn5lurplezyp894y";
        let _permit_name = "default";
        let _chain_id = "secretdev-1";
        let _pub_key = "AkZqxdKMtPq2w0kGDGwWGejTAed0H7azPMHtrCX0XYZG";
        let _signature = "ZXyFMlAy6guMG9Gj05rFvcMi5/JGfClRtJpVTHiDtQY3GtSfBHncY70kmYiTXkKIxSxdnh/kS8oXa+GSX5su6Q==";

        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        let owner = deps.api.addr_validate(user_address).unwrap();
        let payload = String::from("{\"file\": \"content\"}");

        // Mock a new file for a user A
        let _ = store_new_file(deps.as_mut(), owner.clone(), payload);

        let file_ids = query_file_ids(deps.as_ref(), owner.clone()).unwrap();
        assert_eq!(file_ids.file_ids.len(), 1);

        let file_id = &file_ids.file_ids[0];

        println!("Here the key {:?}", file_id);

        // TODO :: Query file ok for owner

        // TODO :: Query file unauthorize user error
        



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

    // TODO :: ~/Project/examples/EVM-encrypt-decrypt/secret_network

    
}
