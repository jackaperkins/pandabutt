use crate::node::ButtNode;
use crate::operation::{ButtEvent, ButtExtensions};
use crate::topic;
use crate::utils::CombinedMigrationSource;

use p2panda_core::PublicKey;
use p2panda_core::{Body, Header, PrivateKey};
use p2panda_store::sqlite::store::{
    connection_pool, create_database, migrations as operation_store_migrations, Pool,
};
use p2panda_store::{LocalOperationStore, LogStore, SqliteStore};
use serde::{Deserialize, Serialize};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use std::hash::Hash as StdHash;
use std::time::SystemTime;
use tokio::sync::mpsc::{self};
use anyhow::Result;

#[derive(Serialize, Deserialize, Clone, Debug, Copy, Eq, PartialEq, StdHash)]
pub struct ButtLogId(pub PublicKey);

pub type OperationStore = SqliteStore<ButtLogId, ButtExtensions>;

#[derive(Clone, Debug)]
pub struct AppData {
    pub pool: sqlx::SqlitePool,
}

impl AppData {
    async fn new(connection_pool: Pool) -> Self {
        AppData {
            pool: connection_pool,
        }
    }

    pub async fn materialize(&self, event: &ButtEvent, header: &Header<ButtExtensions>) {
        println!("Materializing event");
        match event {
            ButtEvent::Follow(_) => {
                // app_data
                //     .friends
                //     .entry(*friend_key)
                //     .and_modify(|public_keys| {
                //         public_keys.insert(*friend_key);
                //     });
            }
            ButtEvent::Post(body) => {
                let _result = sqlx::query(
                    "
                    INSERT OR IGNORE INTO posts ( id, public_key, timestamp, body )
                    VALUES ( ?, ?, ?, ? )
                    ",
                )
                .bind(header.hash().to_string())
                .bind(header.public_key.to_string())
                .bind(header.timestamp.to_string())
                .bind(body)
                .execute(&self.pool)
                .await;
                // let post = ButtPost {
                //     body: body.clone(),
                //     public_key: header.public_key,
                //     id: header.hash(),
                //     timestamp: header.timestamp,
                // };

                // app_data
                //     .posts
                //     .entry(header.public_key)
                //     .and_modify(|posts| {
                //         posts.insert(post.clone());
                //     })
                //     .or_insert(HashSet::from([post]));
                // drop(app_data);
            }
        }
        // self.save().await;
    }

    pub async fn get_posts(&self) -> Vec<FrontendPost> {
        let posts: Vec<SqliteRow> =
            sqlx::query("SELECT id, public_key, timestamp, body FROM posts")
                .fetch_all(&self.pool)
                .await
                .unwrap_or(vec![]);

        posts
            .iter()
            .filter_map(|row| {
                let Ok(id) = row.try_get::<String, _>("id") else {
                    return None;
                };
                let Ok(public_key) = row.try_get::<String, _>("public_key") else {
                    return None;
                };
                let Ok(timestamp) = row.try_get::<u64, _>("timestamp") else {
                    return None;
                };
                let Ok(body) = row.try_get::<String, _>("body") else {
                    return None;
                };
                Some(FrontendPost {
                    id,
                    public_key,
                    timestamp,
                    body,
                })
            })
            .collect()
    }

    pub async fn get_all_keys(&self) -> Vec<PublicKey> {
        let unique_keys: Vec<SqliteRow> = sqlx::query("SELECT DISTINCT public_key FROM posts")
            .fetch_all(&self.pool)
            .await
            .unwrap_or(vec![]);
        unique_keys
            .iter()
            .filter_map(|row| {
                let Ok(public_key) = row.try_get::<&[u8], _>("public_key") else {
                    return None;
                };
                PublicKey::try_from(public_key).ok()
            })
            .collect()
    }
}

#[derive(Serialize)]
pub struct FrontendPost {
    id: String,
    public_key: String,
    timestamp: u64,
    body: String,
}

pub struct Backend {
    #[allow(dead_code)]
    node: ButtNode,
    pub private_key: PrivateKey,
    store: OperationStore,
    pub app_data: AppData,
}

impl Backend {
    pub async fn new(private_key: PrivateKey, data_path: String) -> Result<Self> {
        let operation_db_path = format!("{}/operations.db", data_path);
        create_database(&operation_db_path)
            .await
            .expect("database file created");
        let connection_pool = connection_pool(&operation_db_path, 4)
            .await
            .unwrap_or_else(|_| panic!("database to exist at {}", &operation_db_path));

        Migrator::new(CombinedMigrationSource::new(vec![
            operation_store_migrations(),
            sqlx::migrate!(),
        ]))
        .await?
        .run(&connection_pool)
        .await?;

        let store = OperationStore::new(connection_pool.clone());

        let (tx, mut rx_from_sync) = mpsc::channel::<(Header<ButtExtensions>, Body)>(10000);

        let app_data = AppData::new(connection_pool.clone()).await;
        let topic_map = topic::ButtLogMap::new(store.clone(), app_data.clone());

        let public_key = private_key.public_key();

        let backend = Backend {
            node: ButtNode::new(store.clone(), private_key.clone(), tx, topic_map.clone()).await,
            private_key,
            store: store.clone(),
            app_data: app_data.clone(),
        };

        // kick off rx loop in backend?
        tokio::task::spawn(async move {
            let mut store = store;

            while let Some((header, body)) = rx_from_sync.recv().await {
                println!("Event arrived from sync into materializer ✏️");
                let _ = store
                    .insert_operation(
                        header.hash(),
                        &header,
                        Some(&body),
                        &header.to_bytes(),
                        &ButtLogId(public_key),
                    )
                    .await;

                let butt_event = ButtEvent::from_bytes(body.to_bytes());
                app_data.materialize(&butt_event, &header).await;
            }
        });

        Ok(backend)
    }

    pub async fn create_operation(&mut self, body: &[u8]) -> (Header<ButtExtensions>, Body) {
        let body = Body::new(body);
        let public_key = self.private_key.public_key();

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time from operation system")
            .as_secs();

        let log_id = ButtLogId(public_key);

        let latest_operation = self
            .store
            .latest_operation(&public_key, &log_id)
            .await
            .unwrap();

        let (seq_num, backlink) = match latest_operation {
            Some((header, _)) => (header.seq_num + 1, Some(header.hash())),
            None => (0, None),
        };

        let mut header = Header {
            version: 1,
            public_key,
            signature: None,
            payload_size: body.size(),
            payload_hash: Some(body.hash()),
            timestamp,
            seq_num,
            backlink,
            previous: vec![],
            extensions: Some(ButtExtensions::default()),
        };
        header.sign(&self.private_key);

        (header, body)
    }

    pub async fn insert_operation(
        &mut self,
        body: &Body,
        header: &Header<ButtExtensions>,
        event: &ButtEvent,
    ) {
        let _ = self
            .store
            .insert_operation(
                header.hash(),
                header,
                Some(body),
                &header.to_bytes(),
                &ButtLogId(self.private_key.public_key()),
            )
            .await;

        self.app_data.materialize(event, header).await;
    }

    #[allow(dead_code)]
    pub async fn follow(&mut self, friend_key: PublicKey) -> (ButtEvent, Header<ButtExtensions>) {
        println!("Following my new friend: {}", friend_key);
        let follow = ButtEvent::Follow(friend_key);
        let (header, body) = self.create_operation(&follow.to_bytes()).await;

        self.insert_operation(&body, &header, &follow).await;

        (follow, header)
    }

    pub async fn create_post(&mut self, post_body: String) -> (ButtEvent, Header<ButtExtensions>) {
        println!("Creating a post!");
        let post = ButtEvent::Post(post_body);
        let (header, body) = self.create_operation(&post.to_bytes()).await;

        self.insert_operation(&body, &header, &post).await;
        self.node.send_gossip(header.clone(), body.clone()).await;
        (post, header)
    }
}
