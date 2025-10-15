use crate::{r#async::RawResponse, Error};
use bitcoin_ohttp as ohttp;
use bitreq::{Method, Request};
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
        let res = Request::new(Method::Get, gateway_url.as_str())
            .send_async()
            .await?;
        let key_config = ohttp::KeyConfig::decode(res.as_bytes()).map_err(Error::Ohttp)?;
        Ok(Self {
            key_config,
            relay_url: Url::parse(relay_url).map_err(Error::UrlParsing)?,
        })
    }

    pub(crate) fn relay_url(&self) -> &Url {
        &self.relay_url
    }

    pub(crate) fn encapsulate(
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

    pub(crate) fn decapsulate(
        &self,
        res_ctx: ohttp::ClientResponse,
        body: Vec<u8>,
    ) -> Result<RawResponse, Error> {
        let bhttp_response = res_ctx.decapsulate(&body).map_err(Error::Ohttp)?;
        let mut reader = std::io::Cursor::new(bhttp_response);
        let message = bhttp::Message::read_bhttp(&mut reader).map_err(Error::Bhttp)?;
        let status_code = message
            .control()
            .status()
            .ok_or(Error::InvalidResponse)?
            .into();
        Ok(RawResponse {
            status_code,
            body: message.content().to_vec(),
        })
    }
}
