use ethers::types::{Address, H256};
use ethers::abi::{self, Token};
use ethers::utils::keccak256;
use ethers::signers::{LocalWallet, Signer};
use std::str::FromStr;
use anyhow::{anyhow, Result};

const ERC1967_PREFIX: [u8; 10] = [0x61, 0x00, 0x3d, 0x3d, 0x81, 0x60, 0x23, 0x3d, 0x39, 0x73];

fn deposit_wallet_args(owner: Address, factory: Address) -> Vec<u8> {
    let mut wallet_id = [0u8; 32];
    wallet_id[12..32].copy_from_slice(owner.as_bytes());
    abi::encode(&[
        Token::Address(factory),
        Token::FixedBytes(wallet_id.to_vec()),
    ])
}

fn init_code_hash_erc1967(implementation: Address, args: &[u8]) -> H256 {
    let n = args.len();
    
    // combined = ERC1967_PREFIX + (n << 56)
    let mut prefix_val: u128 = 0;
    for &b in &ERC1967_PREFIX {
        prefix_val = (prefix_val << 8) | (b as u128);
    }
    let combined_val = prefix_val + ((n as u128) << 56);
    let mut combined_bytes = [0u8; 10];
    for i in (0..10).rev() {
        combined_bytes[i] = ((combined_val >> (8 * (9 - i))) & 0xff) as u8;
    }

    let erc1967_const1 = {
        let bytes = hex::decode("cc3735a920a3ca505d382bbc545af43d6000803e6038573d6000fd5b3d6000f3").unwrap();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        arr
    };
    let erc1967_const2 = {
        let bytes = hex::decode("5155f3363d3d373d3d363d7f360894a13ba1a3210667c828492db98dca3e2076").unwrap();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        arr
    };

    let mut init_code = Vec::new();
    init_code.extend_from_slice(&combined_bytes);
    init_code.extend_from_slice(implementation.as_bytes());
    init_code.extend_from_slice(&[0x60, 0x09]);
    init_code.extend_from_slice(&erc1967_const2);
    init_code.extend_from_slice(&erc1967_const1);
    init_code.extend_from_slice(args);

    H256::from(keccak256(&init_code))
}

pub fn derive_uups_deposit_wallet(owner: Address, factory: Address, implementation: Address) -> Address {
    let args = deposit_wallet_args(owner, factory);
    let salt = keccak256(&args);
    let bytecode_hash = init_code_hash_erc1967(implementation, &args);

    let mut data = Vec::new();
    data.push(0xff);
    data.extend_from_slice(factory.as_bytes());
    data.extend_from_slice(&salt);
    data.extend_from_slice(bytecode_hash.as_bytes());

    let hash = keccak256(&data);
    Address::from_slice(&hash[12..32])
}

pub fn derive_eoa_from_private_key(private_key_hex: &str) -> Result<Address> {
    let pk_clean = private_key_hex.trim().trim_start_matches("0x");
    let wallet = LocalWallet::from_str(pk_clean)
        .map_err(|e| anyhow!("Failed to parse private key: {e}"))?;
    Ok(wallet.address())
}

pub fn get_default_deposit_wallet_for_eoa(owner: Address) -> Address {
    let factory = Address::from_str("0x00000000000Fb5C9ADea0298D729A0CB3823Cc07").unwrap();
    let implementation = Address::from_str("0x58CA52ebe0DadfdF531Cde7062e76746de4Db1eB").unwrap();
    derive_uups_deposit_wallet(owner, factory, implementation)
}
