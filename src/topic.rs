use std::collections::HashMap;

use p2panda_core::{Hash, PublicKey};
use p2panda_net::TopicId;
use p2panda_sync::{log_sync::TopicLogMap, TopicQuery};
use serde::{Deserialize, Serialize};

use crate::backend::{AppData, ButtLogId, OperationStore};

type Logs = HashMap<PublicKey, Vec<ButtLogId>>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ButtQuery {
    pub hops: u8,
}

impl TopicQuery for ButtQuery {}

impl TopicId for ButtQuery {
    fn id(&self) -> [u8; 32] {
        Hash::new("gossip-topic").into()
    }
}

#[derive(Debug, Clone)]
pub struct ButtLogMap {
    #[allow(unused)]
    store: OperationStore,
    app_data: AppData
}

impl ButtLogMap {
    pub fn new(s: OperationStore, app_data: AppData) -> Self {
        ButtLogMap {
            store: s,
            app_data
        }
    }
}

#[async_trait]
impl TopicLogMap<ButtQuery, ButtLogId> for ButtLogMap {
    async fn get(&self, _topic: &ButtQuery) -> Option<Logs> {
        // self.store
        // let app_data = self.app_data.inner.read().await;
        // let public_keys: Vec<String> = vec![];//pp_data.posts.keys();
        // let mut result = HashMap::new();
        // for public_key in public_keys {
        //     result.insert(*public_key, vec![ButtLogId(*public_key)]);
        // }
        // Some(result)
        None
    }
}