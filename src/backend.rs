use crate::node::ButtNode;
use crate::operation::{ButtEvent, ButtExtensions, ButtPost};
use crate::topic;
use crate::utils::CombinedMigrationSource;
use p2panda_core::PublicKey;
use p2panda_core::{Body, Header, PrivateKey};
use p2panda_store::sqlite::store::{
    connection_pool, create_database, migrations as operation_store_migrations,
};
use p2panda_store::{LocalOperationStore, LogStore, SqliteStore};
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::{migrate::Migrator, sqlite};
use std::collections::{HashMap, HashSet};
use std::hash::Hash as StdHash;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::mpsc::{self};

use anyhow::Result;

use tokio::fs;
use tokio::sync::{Mutex, RwLock};

#[derive(Serialize, Deserialize, Clone, Debug, Copy, Eq, PartialEq, StdHash)]
pub struct ButtLogId(pub PublicKey);

pub type ButtStore = SqliteStore<ButtLogId, ButtExtensions>;

#[derive(Clone, Debug)]
pub struct AppData {
    pub inner: Arc<RwLock<AppDataInner>>,
    pub db_path: Arc<Mutex<String>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppDataInner {
    pub friends: HashMap<PublicKey, HashSet<PublicKey>>,
    pub posts: HashMap<PublicKey, HashSet<ButtPost>>,
}

impl AppData {
    async fn new(save_directory: &String) -> Self {
        let save_directory = format!("{}/appdb.json", save_directory);
        let app = AppData {
            inner: Arc::new(RwLock::new(AppDataInner {
                friends: HashMap::new(),
                posts: HashMap::new(),
            })),
            db_path: Arc::new(Mutex::new(save_directory.to_owned())),
        };
        app.load().await;
        app
    }

    async fn load(&self) {
        let location = self.db_path.lock().await;
        let location_path = std::path::Path::new(location.as_str());
        let inner_from_file = match fs::read_to_string(location_path).await {
            Ok(text) => serde_json::from_str::<AppDataInner>(&text).unwrap(),
            Err(_) => Default::default(),
        };

        let mut inner = self.inner.write().await;
        *inner = inner_from_file;
    }

    async fn save(&self) {
        // println!("saving");
        let data = self.inner.read().await;
        let output =
            serde_json::to_string_pretty::<AppDataInner>(&data).expect("app data is serializable");
        // println!("outout: {}", output);
        drop(data);
        let location = self.db_path.lock().await;
        // println!("writing file to {}", location);
        let location = std::path::Path::new(location.as_str());
        fs::write(location, output).await.unwrap();
    }

    pub async fn materialize(&self, event: &ButtEvent, header: &Header<ButtExtensions>) {
        println!("Materializing event");
        let mut app_data = self.inner.write().await;
        match event {
            ButtEvent::Follow(friend_key) => {
                app_data
                    .friends
                    .entry(*friend_key)
                    .and_modify(|public_keys| {
                        public_keys.insert(*friend_key);
                    });
                // .or_insert(HashSet::from([friend_key]));
            }
            ButtEvent::Post(body) => {
                let post = ButtPost {
                    body: body.clone(),
                    public_key: header.public_key,
                    id: header.hash(),
                    timestamp: header.timestamp,
                };

                app_data
                    .posts
                    .entry(header.public_key)
                    .and_modify(|posts| {
                        posts.insert(post.clone());
                    })
                    .or_insert(HashSet::from([post]));
                drop(app_data);
            }
        }
        self.save().await;
    }
    // async fn add_friend(&self) {
    //     let store = self.inner.write().await;
    //     store.friends.insert(bla, blub);
    // }
}

pub struct Backend {
    #[allow(dead_code)]
    node: ButtNode,
    pub private_key: PrivateKey,
    store: ButtStore,
    pub app_data: AppData,
}

impl Backend {
    pub async fn new(private_key: PrivateKey, data_path: String) -> Result<Self> {
        let operation_db_path = format!("{}/operations.db", data_path);
        println!("expected db path: '{}'", &operation_db_path);
        create_database(&operation_db_path)
            .await
            .expect("database file created");
        let connection_pool = connection_pool(&operation_db_path, 4)
            .await
            .expect(&format!("database to exist at {}", &operation_db_path));

        Migrator::new(CombinedMigrationSource::new(vec![
            operation_store_migrations(),
            sqlx::migrate!(),
        ]))
        .await?
        .run(&connection_pool)
        .await?;

        let store = ButtStore::new(connection_pool);

        let (tx, mut rx_from_sync) = mpsc::channel::<(Header<ButtExtensions>, Body)>(10000);

        let app_data = AppData::new(&data_path).await;
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
