use ethers::signers::{LocalWallet, Signer};
use std::str::FromStr;

#[tokio::main]
async fn main() {
    let wallet = LocalWallet::from_str("0x4ea996d3030091be9e6e9dce3d627c49e945bcdb790893a3d6fe6fc50acfc618").unwrap();
    let hash = hex::decode("71e5e125832001e4fe91d45b50973a02e4233b85875a243d04519c0e63b9bdf5").unwrap();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&hash);
    let sig = wallet.sign_hash(arr.into()).unwrap();
    println!("0x{}", sig);
}
