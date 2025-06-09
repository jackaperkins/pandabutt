mod backend;
mod node;
mod operation;
mod topic;
mod utils;

use backend::Backend;
use p2panda_core::PrivateKey;
use p2panda_core::PublicKey;
use rocket::config::LogLevel;
use rocket::fs::FileServer;
use rocket::serde::json::Json;
use rocket::State;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::Read;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::backend::FrontendPost;

#[macro_use]
extern crate rocket;

#[get("/posts")]
async fn api_posts(state: &State<Arc<Mutex<Backend>>>) -> Json<Vec<FrontendPost>> {
    let backend = state.lock().await;
    let posts = backend.app_data.get_posts().await;
    Json(posts)
}

#[derive(Serialize)]
struct Identity {
    public_key: PublicKey
}

#[get("/id")]
async fn api_id(state: &State<Arc<Mutex<Backend>>>) -> Json<Identity> {
    let backend = state.lock().await;
    Json(Identity { public_key: backend.private_key.public_key()})
}


#[derive(Deserialize, Debug)]
struct PostBodyInput {
    body: String,
}

#[post("/post", data = "<input>")]
async fn api_make_post(input: Json<PostBodyInput>, state: &State<Arc<Mutex<Backend>>>) -> &'static str {
    let mut backend = state.lock().await;

    backend.create_post(input.body.to_owned()).await;
    "created a new post"
}

#[launch]
async fn main2() -> _ {
    let args: Vec<String> = env::args().collect();

    let name = args.get(1).expect("Needs name argument (position 1)");
    println!("starting node for 'name' {}", name);

    let data_directory = format!("./keys/{}", name);
    let private_key = get_key(&data_directory);

    println!("key {}", private_key);

    let backend = Backend::new(private_key, data_directory).await.expect("backend up be startable");
    let state = Arc::new(Mutex::new(backend));

    let port = match name.as_str() {
        "a" => 8000,
        _ => 9000,
    };

    rocket::build()
        .manage(state)
        .configure(
            rocket::Config::figment()
                .merge(("port", port))
                .merge(("log_level", LogLevel::Critical)),
        )
        .mount("/", FileServer::new("public", rocket::fs::Options::Index))
        .mount("/", routes![api_id, api_posts, api_make_post])

    // tokio::signal::ctrl_c().await.unwrap();
}

fn get_key(dir: &String) -> PrivateKey {
    let _ = fs::DirBuilder::new().create(dir);
    let file_path = format!("{}/key", dir);

    if let Ok(mut file) = fs::File::open(&file_path) {
        let mut bytes = [0u8; 32];

        if let Ok(()) = file.read_exact(&mut bytes) {
            println!("found a key");
            // found a key and read it into our byes
            return PrivateKey::from_bytes(&bytes);
        };
    }

    println!("did not find a key! creating a key");
    // create one!
    let new_key = PrivateKey::new();
    let _ = fs::write(&file_path, new_key.as_bytes());
    new_key
}
