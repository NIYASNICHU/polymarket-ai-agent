use ethers::types::{Address, H256};
use ethers::abi::{self, Token};
use ethers::utils::keccak256;
use std::str::FromStr;

const ERC1967_PREFIX: [u8; 10] = [0x61, 0x00, 0x3d, 0x3d, 0x81, 0x60, 0x23, 0x3d, 0x39, 0x73];

fn decode_hex_32(s: &str) -> [u8; 32] {
    let bytes = hex::decode(s).unwrap();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr
}

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
    let mut prefix = ERC1967_PREFIX;
    
    // In python: combined = ERC1967_PREFIX + (n << 56)
    // combined.to_bytes(10, "big")
    // Let's do this mathematically in Rust using u128
    let mut prefix_val: u128 = 0;
    for &b in &ERC1967_PREFIX {
        prefix_val = (prefix_val << 8) | (b as u128);
    }
    let combined_val = prefix_val + ((n as u128) << 56);
    let mut combined_bytes = [0u8; 10];
    for i in (0..10).rev() {
        combined_bytes[i] = ((combined_val >> (8 * (9 - i))) & 0xff) as u8;
    }

    println!("Combined bytes from Rust: {:02x?}", combined_bytes);

    let erc1967_const1 = decode_hex_32("cc3735a920a3ca505d382bbc545af43d6000803e6038573d6000fd5b3d6000f3");
    let erc1967_const2 = decode_hex_32("5155f3363d3d373d3d363d7f360894a13ba1a3210667c828492db98dca3e2076");

    let mut init_code = Vec::new();
    init_code.extend_from_slice(&combined_bytes);
    init_code.extend_from_slice(implementation.as_bytes());
    init_code.extend_from_slice(&[0x60, 0x09]);
    init_code.extend_from_slice(&erc1967_const2);
    init_code.extend_from_slice(&erc1967_const1);
    init_code.extend_from_slice(args);

    H256::from(keccak256(&init_code))
}

fn derive_uups_deposit_wallet(owner: Address, factory: Address, implementation: Address) -> Address {
    let args = deposit_wallet_args(owner, factory);
    println!("Rust args: {}", hex::encode(&args));
    let salt = keccak256(&args);
    println!("Rust salt: {}", hex::encode(&salt));
    let bytecode_hash = init_code_hash_erc1967(implementation, &args);
    println!("Rust bytecode_hash: {:?}", bytecode_hash);

    let mut data = Vec::new();
    data.push(0xff);
    data.extend_from_slice(factory.as_bytes());
    data.extend_from_slice(&salt);
    data.extend_from_slice(bytecode_hash.as_bytes());

    let hash = keccak256(&data);
    Address::from_slice(&hash[12..32])
}

fn main() {
    let owner = Address::from_str("0xa1e31458fec7b0b941d7c451c1aa5c767aaa5a01").unwrap();
    let factory = Address::from_str("0x00000000000Fb5C9ADea0298D729A0CB3823Cc07").unwrap();
    let implementation = Address::from_str("0x58CA52ebe0DadfdF531Cde7062e76746de4Db1eB").unwrap();

    let derived = derive_uups_deposit_wallet(owner, factory, implementation);
    println!("RUST DERIVED DEPOSIT WALLET: {:?}", derived);
}
