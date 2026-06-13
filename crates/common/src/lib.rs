// crates/common/src/lib.rs

pub mod db;
pub mod features;
pub mod models;
pub mod repo;
pub mod schema;
pub mod derivation;

// Re-export the types trainer and agent use directly
// so they can write `common::RawMarket` instead of `common::features::RawMarket`
pub use features::{extract_features, extract_label, FeatureVector, RawMarket};
pub use derivation::{derive_uups_deposit_wallet, derive_eoa_from_private_key, get_default_deposit_wallet_for_eoa};

