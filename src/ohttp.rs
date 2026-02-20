use crate::Error;
use bitcoin_ohttp as ohttp;
use reqwest::Client;
use url::Url;

#[derive(Debug, Clone)]
pub struct OhttpClient {
    key_config: ohttp::KeyConfig,
    relay_url: Url,
}

impl OhttpClient {
    /// Will attempt to fetch the key config from the gateway and then create a new client.
    /// Keyconfig is fetched directly from the gateway thus revealing our network metadata.
    /// TODO: use the relay HTTP connect proxy to fetch to.
    pub(crate) async fn new(relay_url: &str, ohttp_gateway_url: &str) -> Result<Self, Error> {
        let gateway_url = Url::parse(ohttp_gateway_url).map_err(Error::UrlParsing)?;
        let res = Client::new()
            .get(gateway_url)
            .send()
            .await
            .map_err(Error::Reqwest)?;
        let body = res.bytes().await.map_err(Error::Reqwest)?;
        let key_config = ohttp::KeyConfig::decode(&body).map_err(Error::Ohttp)?;
        Ok(Self {
            key_config,
            relay_url: Url::parse(relay_url).map_err(Error::UrlParsing)?,
        })
    }

    pub(crate) fn relay_url(&self) -> &Url {
        &self.relay_url
    }

    pub(crate) fn ohttp_encapsulate(
        &self,
        method: &str,
        target_resource: &str,
        body: Option<&[u8]>,
    ) -> Result<(Vec<u8>, ohttp::ClientResponse), Error> {
        use std::fmt::Write;

        // Bitcoin-hpke takes keyconfig as mutable ref but it doesnt mutate it should fix it
        // upstream but for now we can clone it to avoid changing self to mutable self
        let mut key_config = self.key_config.clone();

        let ctx = ohttp::ClientRequest::from_config(&mut key_config).map_err(Error::Ohttp)?;
        let url = url::Url::parse(target_resource).map_err(Error::UrlParsing)?;
        let authority_bytes = url.host().map_or_else(Vec::new, |host| {
            let mut authority = host.to_string();
            if let Some(port) = url.port() {
                write!(authority, ":{port}").unwrap();
            }
            authority.into_bytes()
        });
        let mut bhttp_message = bhttp::Message::request(
            method.as_bytes().to_vec(),
            url.scheme().as_bytes().to_vec(),
            authority_bytes,
            url.path().as_bytes().to_vec(),
        );
        // TODO: do we need to add headers?
        if let Some(body) = body {
            bhttp_message.write_content(body);
        }

        let mut bhttp_req = Vec::new();
        bhttp_message
            .write_bhttp(bhttp::Mode::IndeterminateLength, &mut bhttp_req)
            .map_err(Error::Bhttp)?;
        let (encapsulated, ohttp_ctx) = ctx.encapsulate(&bhttp_req).map_err(Error::Ohttp)?;

        Ok((encapsulated, ohttp_ctx))
    }

    pub(crate) fn ohttp_decapsulate(
        &self,
        res_ctx: ohttp::ClientResponse,
        ohttp_body: Vec<u8>,
    ) -> Result<http::Response<Vec<u8>>, Error> {
        let bhttp_body = res_ctx.decapsulate(&ohttp_body).map_err(Error::Ohttp)?;
        let mut r = std::io::Cursor::new(bhttp_body);
        let m: bhttp::Message = bhttp::Message::read_bhttp(&mut r).map_err(Error::Bhttp)?;
        let mut builder = http::Response::builder();
        for field in m.header().iter() {
            builder = builder.header(field.name(), field.value());
        }
        builder
            .status({
                let code = m
                    .control()
                    .status()
                    .ok_or(bhttp::Error::InvalidStatus)
                    .map_err(Error::Bhttp)?;
                http::StatusCode::from_u16(code.code())
                    .map_err(|_| bhttp::Error::InvalidStatus)
                    .map_err(Error::Bhttp)?
            })
            .body(m.content().to_vec())
            .map_err(Error::Http)
    }
}
