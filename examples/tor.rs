use std::str::FromStr;

use bitcoin::Txid;
use esplora_client::{r#async_tor::AsyncTorClient, Builder};

extern crate esplora_client;

#[tokio::main]
async fn main() {
    // let client = Client::new("tcp://electrum.blockstream.info:50001").unwrap();
    // let res = client.server_features();
    // println!("{:#?}", res);

    let builder = Builder::new("https://mempool.space/api");
    let client = AsyncTorClient::from_builder(builder).await.unwrap();

    // let client = AsyncClient::from_builder(builder).unwrap();

    let tx_id =
        Txid::from_str("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b").unwrap();
    let tx = client.get_tx(&tx_id).await.unwrap().unwrap();

    println!("{:?}", tx);
}
