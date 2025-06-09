use crate::backend::ButtLogId;
use anyhow::Result;
use p2panda_core::cbor::encode_cbor;
use p2panda_core::{Body, Extension, Extensions, Header, PruneFlag};
use p2panda_core::{Hash, PublicKey};
use serde::{Deserialize, Serialize};
use std::hash::Hash as StdHash;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ButtExtensions {
    pub prune_flag: PruneFlag,
}

impl Default for ButtExtensions {
    fn default() -> Self {
        ButtExtensions {
            prune_flag: PruneFlag::new(false),
        }
    }
}

impl Extension<PruneFlag> for ButtExtensions {
    fn extract(header: &Header<Self>) -> Option<PruneFlag> {
        header
            .extensions
            .as_ref()
            .map(|extensions| extensions.prune_flag.clone())
    }
}

impl Extension<ButtLogId> for ButtExtensions {
    fn extract(header: &Header<Self>) -> Option<ButtLogId> {
        Some(ButtLogId(header.public_key))
    }
}

#[derive(StdHash, PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct ButtPost {
    pub id: Hash,
    pub public_key: PublicKey,
    pub timestamp: u64,
    pub body: String,
}

#[derive(Serialize, Deserialize)]
pub enum ButtEvent {
    Post(String),
    Follow(PublicKey),
}

impl ButtEvent {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self).expect("message converted to json")
    }
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        serde_json::from_slice(&bytes).unwrap()
    }
}

pub fn encode_gossip_operation<E>(header: Header<E>, body: Option<Body>) -> Result<Vec<u8>>
where
    E: Extensions + Serialize,
{
    let bytes = encode_cbor(&(header.to_bytes(), body.map(|body| body.to_bytes())))?;
    Ok(bytes)
}
