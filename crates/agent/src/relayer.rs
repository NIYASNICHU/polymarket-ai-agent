use anyhow::{anyhow, Result};
use ethers::{
    abi::{encode, Token},
    signers::{LocalWallet, Signer},
    types::{Address, U256},
    utils::keccak256,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, warn};

#[derive(Debug, Serialize)]
pub struct ProxyTransaction {
    pub to: String,
    pub data: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
struct RelayerConfig {
    #[serde(rename = "relayHub")]
    relay_hub: String,
    #[serde(rename = "proxyFactory")]
    proxy_factory: String,
}

#[derive(Debug, Deserialize)]
struct RelayPayloadResponse {
    nonce: String,
    address: String,
}

#[derive(Debug, Serialize)]
struct SignatureParams {
    #[serde(rename = "gasPrice")]
    gas_price: String,
    #[serde(rename = "gasLimit")]
    gas_limit: String,
    #[serde(rename = "relayerFee")]
    relayer_fee: String,
    #[serde(rename = "relayHub")]
    relay_hub: String,
    relay: String,
}


#[derive(Debug, Serialize)]
struct RelayerSubmitRequest {
    #[serde(rename = "type")]
    tx_type: String,
    from: String,
    to: String,
    #[serde(rename = "proxyWallet")]
    proxy_wallet: String,
    data: String,
    nonce: String,
    signature: String,
    #[serde(rename = "signatureParams")]
    signature_params: SignatureParams,
    value: Option<String>,
    metadata: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RelayerSubmitResponse {
    #[serde(rename = "transactionHash")]
    transaction_hash: Option<String>,
    error: Option<String>,
}

pub async fn init_approvals(
    client: &Client,
    relayer_api_key: &str,
    relayer_api_key_address: &str,
    private_key: &str,
) -> Result<()> {
    if relayer_api_key.is_empty() || relayer_api_key_address.is_empty() {
        warn!("RELAYER_API_KEY or RELAYER_API_KEY_ADDRESS not set, skipping gasless approvals.");
        return Ok(());
    }

    let wallet = LocalWallet::from_str(private_key)
        .map_err(|e| anyhow!("Invalid private key: {e}"))?;
    let from_address_hex = ethers::utils::to_checksum(&wallet.address(), None);

    info!(
        from_address = %from_address_hex,
        "Fetching proxy config from Relayer API"
    );

    let config = RelayerConfig {
        proxy_factory: "0xaB45c5A4B0c941a2F231C04C3f49182e1A254052".to_string(),
        relay_hub: "0xD216153c06E857cD7f72665E0aF1d7D82172F494".to_string(),
    };

    let nonce_resp: RelayPayloadResponse = client
        .get(format!(
            "https://relayer-v2.polymarket.com/relay-payload?address={}&type=PROXY",
            from_address_hex
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let usdc_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"; // Polygon USDC
    let ctf_exchange = "0x4bFb16f68BE8e31313bf1bE3bEebEa115E039600"; // CTF Exchange

    // 1. approve USDC max for CTF Exchange
    // approve(address spender, uint256 amount)
    let usdc_data = format!(
        "0x095ea7b3000000000000000000000000{}ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        &ctf_exchange[2..].to_lowercase()
    );

    let txs = vec![
        ProxyTransaction {
            to: usdc_address.to_string(),
            data: usdc_data,
            value: "0".to_string(),
        },
    ];

    let encoded_data = encode_proxy_transactions(&txs)?;
    let gas_price = "0";
    let gas_limit = "500000";
    let tx_fee = "0";

    info!(
        "Inputs to create_proxy_struct_hash: from: {}, to: {}, data: {}, tx_fee: {}, gas_price: {}, gas_limit: {}, nonce: {}, relay_hub: {}, relay: {}",
        from_address_hex, config.proxy_factory, encoded_data, tx_fee, gas_price, gas_limit, nonce_resp.nonce, config.relay_hub, nonce_resp.address
    );

    let struct_hash = create_proxy_struct_hash(
        &from_address_hex,
        &config.proxy_factory,
        &encoded_data,
        tx_fee,
        gas_price,
        gas_limit,
        &nonce_resp.nonce,
        &config.relay_hub,
        &nonce_resp.address,
    )?;

    info!("Struct hash: 0x{}", hex::encode(struct_hash));

    let signature = wallet.sign_message(struct_hash).await
        .map_err(|e| anyhow!("Failed to sign proxy hash: {e}"))?;
    
    // Convert signature to 0x... format
    let sig_str = format!("0x{}", signature);
    info!("Generated signature: {}", sig_str);

    let proxy_wallet = "0x585e123CEf250a5771aC598d93eBFA40b0d8b1EC".to_string();

    let submit_req = RelayerSubmitRequest {
        to: config.proxy_factory.clone(),
        data: encoded_data,
        nonce: nonce_resp.nonce.clone(),
        from: from_address_hex,
        tx_type: "PROXY".to_string(),
        proxy_wallet,
        signature: sig_str,
        signature_params: SignatureParams {
            gas_price: gas_price.to_string(),
            gas_limit: gas_limit.to_string(),
            relayer_fee: "0".to_string(),
            relay_hub: config.relay_hub.clone(),
            relay: nonce_resp.address.clone(),
        },
        value: None,
        metadata: None,
    };

    info!("Submitting batch approval to Relayer API...");

    let resp = client
        .post("https://relayer-v2.polymarket.com/submit")
        .header("RELAYER_API_KEY", relayer_api_key)
        .header("RELAYER_API_KEY_ADDRESS", relayer_api_key_address)
        .json(&submit_req)
        .send()
        .await?;

    if !resp.status().is_success() {
        let err_body = resp.text().await?;
        return Err(anyhow!("Relayer submit failed with body: {}", err_body));
    }

    let resp_data: RelayerSubmitResponse = resp.json().await?;

    if let Some(err) = resp_data.error {
        return Err(anyhow!("Relayer API error: {}", err));
    }

    info!(
        tx_hash = ?resp_data.transaction_hash,
        "Successfully submitted gasless approvals"
    );

    Ok(())
}

fn encode_proxy_transactions(txns: &[ProxyTransaction]) -> Result<String> {
    // keccak(b"proxy((uint8,address,uint256,bytes)[])")[:4] = 0xc69622d1
    let function_selector = "c69622d1";

    let mut tuples = Vec::new();
    for tx in txns {
        let to_addr = Address::from_str(&tx.to)
            .map_err(|_| anyhow!("invalid address: {}", tx.to))?;
        let value = U256::from_dec_str(&tx.value)
            .map_err(|_| anyhow!("invalid value: {}", tx.value))?;
        let data_bytes = hex::decode(tx.data.trim_start_matches("0x"))
            .map_err(|_| anyhow!("invalid data hex"))?;

        let tuple = Token::Tuple(vec![
            Token::Uint(U256::from(0)), // Enum type proxy = 0
            Token::Address(to_addr),
            Token::Uint(value),
            Token::Bytes(data_bytes),
        ]);
        tuples.push(tuple);
    }

    let encoded_bytes = encode(&[Token::Array(tuples)]);
    let encoded_hex = hex::encode(encoded_bytes);

    Ok(format!("0x{}{}", function_selector, encoded_hex))
}

fn create_proxy_struct_hash(
    from_address: &str,
    to: &str,
    data: &str,
    tx_fee: &str,
    gas_price: &str,
    gas_limit: &str,
    nonce: &str,
    relay_hub_address: &str,
    relay_address: &str,
) -> Result<[u8; 32]> {
    let mut message = Vec::new();
    message.extend_from_slice(b"rlx:");

    let from_bytes = hex::decode(from_address.trim_start_matches("0x"))?;
    message.extend_from_slice(&from_bytes);

    let to_bytes = hex::decode(to.trim_start_matches("0x"))?;
    message.extend_from_slice(&to_bytes);

    let data_bytes = hex::decode(data.trim_start_matches("0x"))?;
    message.extend_from_slice(&data_bytes);

    let tx_fee_val = u64::from_str(tx_fee)?;
    let mut tx_fee_buf = [0u8; 32];
    tx_fee_buf[24..32].copy_from_slice(&tx_fee_val.to_be_bytes());
    message.extend_from_slice(&tx_fee_buf);

    let gas_price_val = u64::from_str(gas_price)?;
    let mut gas_price_buf = [0u8; 32];
    gas_price_buf[24..32].copy_from_slice(&gas_price_val.to_be_bytes());
    message.extend_from_slice(&gas_price_buf);

    let gas_limit_val = u64::from_str(gas_limit)?;
    let mut gas_limit_buf = [0u8; 32];
    gas_limit_buf[24..32].copy_from_slice(&gas_limit_val.to_be_bytes());
    message.extend_from_slice(&gas_limit_buf);

    let nonce_val = u64::from_str(nonce)?;
    let mut nonce_buf = [0u8; 32];
    nonce_buf[24..32].copy_from_slice(&nonce_val.to_be_bytes());
    message.extend_from_slice(&nonce_buf);

    let relay_hub_bytes = hex::decode(relay_hub_address.trim_start_matches("0x"))?;
    message.extend_from_slice(&relay_hub_bytes);

    let relay_bytes = hex::decode(relay_address.trim_start_matches("0x"))?;
    message.extend_from_slice(&relay_bytes);

    Ok(keccak256(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::signers::{LocalWallet, Signer};
    use std::str::FromStr;

    #[tokio::test]
    async fn test_sig() {
        let wallet = LocalWallet::from_str("0x4ea996d3030091be9e6e9dce3d627c49e945bcdb790893a3d6fe6fc50acfc618").unwrap();
        let hash = hex::decode("71e5e125832001e4fe91d45b50973a02e4233b85875a243d04519c0e63b9bdf5").unwrap();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&hash);
        let sig = wallet.sign_hash(arr.into()).unwrap();
        println!("RUST SIG: 0x{}", sig);
        assert!(false); // to show output
    }
}
