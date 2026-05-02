// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Test Environment
//!
//! The [`TestEnv`] is a regtest testing environment for `rust-esplora-client`.
//!
//! It allows spawning regtest [`BitcoinD`] and [`ElectrsD`] processes,
//! which are then used by this crate's tests to test client methods.

#![allow(unused)]

use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use bitcoin::Address;
use electrsd::bitcoind;
use electrsd::bitcoind::BitcoinD;
use electrsd::electrum_client::ElectrumApi;
use electrsd::ElectrsD;

#[cfg(all(feature = "async", feature = "tokio"))]
use esplora_client::AsyncClient;
#[cfg(feature = "blocking")]
use esplora_client::BlockingClient;
use esplora_client::Builder;

fn setup_bitcoind() -> BitcoinD {
    let mut conf = bitcoind::Conf::default();
    conf.args.push("-txindex=1");
    BitcoinD::from_downloaded_with_conf(&conf).unwrap()
}

fn setup_electrsd(bitcoind: &BitcoinD) -> ElectrsD {
    let mut conf = electrsd::Conf::default();
    conf.http_enabled = true;
    ElectrsD::with_conf(
        electrsd::downloaded_exe_path().expect("electrs not found"),
        bitcoind,
        &conf,
    )
    .unwrap()
}

/// The [`TestEnv`] is composed of:
/// - A Bitcoin Core node, via [`BitcoinD`].
/// - An Electrum server, which also exposes an Esplora HTTP API, via [`ElectrsD`].
pub(crate) struct TestEnv {
    /// The Bitcoin Core node.
    bitcoind: BitcoinD,
    /// The Electrum server associated
    /// with the Bitcoin Core node.
    electrsd: ElectrsD,
    /// The [`AsyncClient`].
    #[cfg(all(feature = "async", feature = "tokio"))]
    pub async_client: AsyncClient,
    /// The [`BlockingClient`].
    #[cfg(feature = "blocking")]
    pub blocking_client: BlockingClient,
}

/// Configuration parameters for the [`TestEnv`].
pub(crate) struct EnvConfig<'a> {
    /// Configuration params for the [`BitcoinD`] node.
    pub(crate) bitcoind: bitcoind::Conf<'a>,
    /// Configuration params for the [`ElectrsD`] server.
    pub(crate) electrsd: electrsd::Conf<'a>,
}

impl Default for EnvConfig<'_> {
    /// Use [`BitcoinD`]'s default configuration,
    /// and enable the [`ElectrsD`]'s Esplora HTTP API.
    fn default() -> Self {
        Self {
            bitcoind: bitcoind::Conf::default(),
            electrsd: {
                let mut config = electrsd::Conf::default();
                config.http_enabled = true;
                config
            },
        }
    }
}

impl TestEnv {
    /// Instantiate a [`TestEnv`] with the default [`EnvConfig`].
    pub fn new() -> Self {
        Self::new_with_config(EnvConfig::default())
    }

    /// Instantiate a [`TestEnv`] with a custom [`EnvConfig`].
    pub(crate) fn new_with_config(config: EnvConfig) -> Self {
        const SETUP_BLOCK_COUNT: usize = 101;

        let bitcoind_exe = std::env::var("BITCOIND_EXE")
            .ok()
            .or_else(|| bitcoind::downloaded_exe_path().ok())
            .expect(
                "Provide a BITCOIND_EXE environment variable, or specify a `bitcoind` version feature",
            );
        let bitcoind = BitcoinD::with_conf(bitcoind_exe, &config.bitcoind).unwrap();

        let electrs_exe = std::env::var("ELECTRS_EXE")
            .ok()
            .or_else(electrsd::downloaded_exe_path)
            .expect(
                "Provide an ELECTRS_EXE environment variable, or specify an `electrsd` version feature",
            );
        let electrsd = ElectrsD::with_conf(electrs_exe, &bitcoind, &config.electrsd).unwrap();

        let base_url = format!("http://{}", electrsd.esplora_url.as_ref().unwrap());

        #[cfg(feature = "blocking")]
        let blocking_client = Builder::new(&base_url)
            .header("User-Agent", "blocking")
            .build_blocking();

        #[cfg(feature = "async")]
        let async_client = Builder::new(&base_url)
            .header("User-Agent", "async")
            .build_async()
            .unwrap();

        let env = Self {
            bitcoind,
            electrsd,
            #[cfg(feature = "blocking")]
            blocking_client,
            #[cfg(feature = "async")]
            async_client,
        };

        env.bitcoind_client()
            .generate_to_address(SETUP_BLOCK_COUNT, &env.get_mining_address())
            .unwrap();
        env.wait_until_electrum_sees_block(SETUP_BLOCK_COUNT);

        env
    }

    /// Get the [`bitcoind` RPC client](bitcoind::Client).
    pub(crate) fn bitcoind_client(&self) -> &bitcoind::Client {
        &self.bitcoind.client
    }

    #[cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]
    /// Setup both [`BlockingClient`] and [`AsyncClient`].
    pub(crate) fn setup_clients(&self) -> (BlockingClient, AsyncClient) {
        self.setup_clients_with_headers(self.electrsd.esplora_url.as_ref().unwrap(), HashMap::new())
    }

    #[cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]
    /// Setup both [`BlockingClient`] and [`AsyncClient`] with custom HTTP headers.
    pub(crate) fn setup_clients_with_headers(
        &self,
        url: &str,
        headers: HashMap<String, String>,
    ) -> (BlockingClient, AsyncClient) {
        let mut builder = Builder::new(&format!("http://{url}"));
        for (k, v) in &headers {
            builder = builder.header(k, v);
        }
        let blocking_client = builder
            .clone()
            .header("User-Agent", "blocking")
            .build_blocking();
        let async_client = builder.header("User-Agent", "async").build_async().unwrap();

        (blocking_client, async_client)
    }

    /// Mine `count` blocks.
    pub(crate) fn mine_blocks(&self, count: usize) {
        self.bitcoind
            .client
            .generate_to_address(count, &self.get_mining_address())
            .unwrap();
    }

    /// Wait until the [electrum server](electrsd::ElectrsD) sees a new block.
    pub(crate) fn wait_until_electrum_sees_block(&self, min_height: usize) {
        let electrsd = &self.electrsd;
        let mut header = electrsd.client.block_headers_subscribe().unwrap();
        loop {
            if header.height >= min_height {
                break;
            }
            header = self.poll_exp_backoff(|| {
                electrsd.trigger().unwrap();
                electrsd.client.ping().unwrap();
                electrsd.client.block_headers_pop().unwrap()
            });
        }
    }

    /// Mine `count` blocks and wait until the
    /// [electrum server](electrsd::ElectrsD) sees a new block.
    pub(crate) fn mine_and_wait(&self, count: usize) {
        let current_height = self
            .electrsd
            .client
            .block_headers_subscribe()
            .unwrap()
            .height;
        self.mine_blocks(count);
        self.wait_until_electrum_sees_block(current_height + count);
    }

    /// Poll the [electrum server](electrsd::ElectrsD) in exponentially increasing intervals.
    fn poll_exp_backoff<T, F>(&self, mut poll: F) -> T
    where
        F: FnMut() -> Option<T>,
    {
        let mut delay = Duration::from_millis(64);
        loop {
            match poll() {
                Some(data) => break data,
                None if delay.as_millis() < 512 => delay = delay.mul_f32(2.0),
                None => {}
            }
            std::thread::sleep(delay);
        }
    }

    /// Get a `Legacy` regtest address.
    pub(crate) fn get_legacy_address(&self) -> Address {
        Address::from_str("mvUsRD2pNeQQ8nZq8CDEx6fjVQsyzqyhVC")
            .unwrap()
            .assume_checked()
    }

    /// Get a `Nested SegWit` (P2SH-P2WSH) regtest address.
    pub(crate) fn get_nested_segwit_address(&self) -> Address {
        Address::from_str("2N2bJevrSwzv5C6dGm9kQAivDYnvDBPbUxM")
            .unwrap()
            .assume_checked()
    }

    /// Get a `bech32` regtest address.
    pub(crate) fn get_bech32_address(&self) -> Address {
        Address::from_str("bcrt1qedegah48k0uft3ez7u8ywg2hf0ygexgvhps0wp")
            .unwrap()
            .assume_checked()
    }

    /// Get a `bech32m` regtest address.
    pub(crate) fn get_bech32m_address(&self) -> Address {
        Address::from_str("bcrt1p970nsjmz8ls34ty229n6zu534mumc2j74skuxe2lzcqdqxuwwhxsftk7al")
            .unwrap()
            .assume_checked()
    }

    /// Get an address which coinbase outputs should be sent to.
    pub(crate) fn get_mining_address(&self) -> Address {
        Address::from_str("bcrt1qj5gx4t0n8lrl0clddmpn0pee4r4fds7stwyj0j")
            .unwrap()
            .assume_checked()
    }
}

#[cfg(test)]
mod test {
    #[cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]
    #[tokio::test]
    async fn test_that_errors_are_propagated() {
        use bitcoin::Amount;
        use esplora_client::Error;

        use crate::TestEnv;

        let env = TestEnv::new();
        let (blocking_client, async_client) = env.setup_clients();

        let address = env.get_legacy_address();
        let txid = env
            .bitcoind_client()
            .send_to_address(&address, Amount::from_sat(1000))
            .unwrap()
            .txid()
            .unwrap();
        env.mine_and_wait(1);

        let tx = blocking_client.get_tx(&txid).unwrap();
        let async_res = async_client.broadcast(tx.as_ref().unwrap()).await;
        println!("{:?}", async_res);
        let blocking_res = blocking_client.broadcast(tx.as_ref().unwrap());
        assert!(async_res.is_err());
        assert!(matches!(
            async_res.unwrap_err(),
            Error::HttpResponse { status: 400, message } if message.contains("-27")
        ));
        assert!(blocking_res.is_err());
        assert!(matches!(
            blocking_res.unwrap_err(),
            Error::HttpResponse { status: 400, message } if message.contains("-27")
        ));
    }
}
