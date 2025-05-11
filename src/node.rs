
use anyhow::Result;
use p2panda_core::{cbor::decode_cbor, Body, Hash, Header, PrivateKey};
use p2panda_discovery::mdns::LocalDiscovery;
use p2panda_net::{FromNetwork, Network, NetworkBuilder, SyncConfiguration, ToNetwork};
use p2panda_stream::{DecodeExt, IngestExt};
use p2panda_sync::log_sync::LogSyncProtocol;

use tokio::sync::mpsc::{self, Sender};
use tokio::task;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::{
    backend::ButtStore,
    operation::{encode_gossip_operation, ButtExtensions},
    topic::{ButtLogMap, ButtQuery},
};

pub struct ButtNode {
    #[allow(unused)]
    network: Network<ButtQuery>,
    gossip_tx: Sender<ToNetwork>
}

impl ButtNode {
    pub async fn new(
        store: ButtStore,
        private_key: PrivateKey,
        backend_tx: mpsc::Sender<(Header<ButtExtensions>, Body)>,
        topic_map: ButtLogMap,
    ) -> Self {
        let mdns = LocalDiscovery::new();

        let network = NetworkBuilder::new(Hash::new(b"butt-net").into())
            .private_key(private_key)
            .discovery(mdns)
            // .direct_address(bootstrap_public_key, vec![], None)
            // .relay("https://wasser.liebechaos.org".parse().unwrap(), false, 0)
            .sync(SyncConfiguration::new(LogSyncProtocol::new(
                topic_map,
                store.clone(),
            )))
            .build()
            .await
            .unwrap();

        let (gossip_tx, rx, gossip_ready) = network.subscribe(ButtQuery { hops: 1 }).await.unwrap();


        let backend_copy = backend_tx.clone();
        task::spawn(async move {
            let stream = ReceiverStream::new(rx);
            let stream = stream.filter_map(|event| match event {
                FromNetwork::GossipMessage { bytes, .. } => match decode_gossip_message(&bytes) {
                    Ok(result) => {
                        println!("got gossip message in! 👂");
                        Some(result)
                    }
                    Err(err) => {
                        error!("could not decode gossip message: {err}");
                        None
                    }
                },
                FromNetwork::SyncMessage {
                    header, payload, ..
                } => {
                    println!("got sync message in!");
                    Some((header, payload))
                }
            });

            // Decode and ingest the p2panda operations.
            let mut stream = stream
                .decode()
                .filter_map(|result| match result {
                    Ok(operation) => {
                        println!("decoded operation ok in filter_map stream");
                        Some(operation)
                    },
                    Err(err) => {
                        error!("decode operation error: {err}");
                        None
                    }
                })
                .ingest(store.clone(), 128)
                .filter_map(|result| match result {
                    Ok(operation) => Some(operation),
                    Err(err) => {
                        error!("ingest operation error: {err}");
                        None
                    }
                });

            // let documents_store = network_inner.documents_store.clone();

            tokio::task::spawn(async move {
                // Process the operations and forward application messages to app layer.
                while let Some(operation) = stream.next().await {
                    // Forward the payload up to the app.
                    println!("sending operation to the app backend");
                    let _r = backend_copy
                        .send((
                            operation.header,
                            operation.body.expect("all operations have bodies"),
                        ))
                        .await;
                }
            });
        });

        task::spawn(async {
            if gossip_ready.await.is_ok() {
                println!("met a peer, ready to gossip");
            }
        });

        ButtNode { network, gossip_tx }
    }

    pub async fn send_gossip (&self, header :Header<ButtExtensions>, body:Body) {
        let encoded = encode_gossip_operation(header.clone(), Some(body.clone())).unwrap();
        let message = ToNetwork::Message {
            bytes: encoded
        };
        println!("gossiping about a new operation to my friends {}", header.hash());
        let _ = self.gossip_tx.send(message).await;
    }
}

pub fn decode_gossip_message(bytes: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    let result = decode_cbor(bytes)?;
    Ok(result)
}
