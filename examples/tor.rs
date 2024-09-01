use bitcoin::{consensus::encode::deserialize_hex, Transaction};
use esplora_client::{r#async_tor::AsyncTorClient, Builder};

extern crate esplora_client;

// const MEMPOOL_SPACE_API: &str = "https://mempool.space/api";
const MEMPOOL_SPACE_API: &str = "https://blockstream.info/api";

#[tokio::main]
async fn main() {
    let builder = Builder::new(MEMPOOL_SPACE_API);
    let esplora_client = AsyncTorClient::from_builder(builder).await.unwrap();

    let raw_tx = "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff4d04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73ffffffff0100f2052a01000000434104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5fac00000000";
    let tx: Transaction = deserialize_hex(raw_tx).unwrap();
    esplora_client.broadcast(&tx).await.unwrap();

    print!(
        "successfully broadcasted transaction, with txid: {:?}",
        tx.compute_txid()
    );

    // let tx_id =
    //     Txid::from_str("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b").unwrap();
    // let tx = esplora_client.get_tx(&tx_id).await.unwrap().unwrap();

    // println!("successfully fetched the transaction {:?}", tx);
}
