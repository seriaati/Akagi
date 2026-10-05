//! Hybrid capture: the MITM proxy and the controlled Chromium at once.
//!
//! The Akagi-launched browser routes all its traffic through the proxy
//! (`--proxy-server`) and accepts the proxy's certificates by SPKI pin, so
//! neither the system proxy nor the OS trust store is touched. The proxy
//! owns frame capture and every rewrite (cosmetic unlock, certificate
//! report, telemetry blocking); CDP is used only to bind the page handle
//! that click-based autoplay drives.

use super::{
    chromium::{launch::ChromiumProxy, ChromiumBackend},
    hudsucker_backend::HudsuckerBackend,
    CaptureBackend, CaptureCtx, CaptureDescriptor, CaptureKind, ShutdownToken,
};
use crate::config::{ChromiumConfig, HttpCaptureConfig, ProxyConfig};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tracing::info;

/// How long to wait for the proxy to bind before launching the browser,
/// whose first navigation would otherwise fail with a proxy error.
const PROXY_LISTEN_TIMEOUT: Duration = Duration::from_secs(10);

pub struct HybridBackend {
    proxy_cfg: ProxyConfig,
    http_cfg: HttpCaptureConfig,
    chromium_cfg: ChromiumConfig,
    force_close: Arc<Notify>,
}

impl HybridBackend {
    pub fn new(
        proxy_cfg: ProxyConfig,
        http_cfg: HttpCaptureConfig,
        chromium_cfg: ChromiumConfig,
        force_close: Arc<Notify>,
    ) -> Self {
        Self {
            proxy_cfg,
            http_cfg,
            chromium_cfg,
            force_close,
        }
    }
}

/// The address the browser should dial. A wildcard listen address is not
/// dialable, so it maps to loopback on the same port.
fn dial_addr(listen: &str) -> Result<SocketAddr> {
    let mut addr: SocketAddr = listen
        .parse()
        .with_context(|| format!("Invalid proxy addr: {listen}"))?;
    if addr.ip().is_unspecified() {
        addr.set_ip(if addr.is_ipv4() {
            std::net::Ipv4Addr::LOCALHOST.into()
        } else {
            std::net::Ipv6Addr::LOCALHOST.into()
        });
    }
    Ok(addr)
}

async fn wait_until_listening(addr: SocketAddr) -> Result<()> {
    let deadline = tokio::time::Instant::now() + PROXY_LISTEN_TIMEOUT;
    while tokio::net::TcpStream::connect(addr).await.is_err() {
        if tokio::time::Instant::now() > deadline {
            anyhow::bail!("proxy did not start listening on {addr}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

#[async_trait]
impl CaptureBackend for HybridBackend {
    async fn run(self: Box<Self>, ctx: CaptureCtx, shutdown: ShutdownToken) -> Result<()> {
        // Before the proxy starts, so the CA exists and nothing races to
        // generate it.
        let spki_pin =
            crate::proxy::ca_spki_pin(&self.proxy_cfg.ca_dir).context("loading proxy CA")?;
        let addr = dial_addr(&self.proxy_cfg.addr)?;
        info!("hybrid backend: chromium via proxy {addr}");

        let mitm: Box<dyn CaptureBackend> = Box::new(
            HudsuckerBackend::new(self.proxy_cfg, self.http_cfg, self.force_close)
                .with_page_autoplay(),
        );
        let chromium: Box<dyn CaptureBackend> = Box::new(
            ChromiumBackend::new(self.chromium_cfg).via_proxy(ChromiumProxy {
                server: addr.to_string(),
                spki_pin,
            }),
        );

        // Each half stops the other: a dead proxy leaves the browser with no
        // network, and a closed browser leaves nothing to capture. One token
        // per half, fired with `notify_one` — it latches, so a half that is
        // still starting up sees the stop once it begins waiting.
        let (mitm_token, mitm_stop) = ShutdownToken::new();
        let (chromium_token, chromium_stop) = ShutdownToken::new();
        let stop_both = || {
            mitm_stop.notify_one();
            chromium_stop.notify_one();
        };
        let mitm_fut = mitm.run(ctx.clone(), mitm_token);
        let chromium_fut = async {
            wait_until_listening(addr).await?;
            chromium.run(ctx, chromium_token).await
        };
        let outer = shutdown.wait();
        tokio::pin!(mitm_fut, chromium_fut, outer);

        tokio::select! {
            _ = &mut outer => {
                stop_both();
                let (m, c) = tokio::join!(mitm_fut, chromium_fut);
                m.context("proxy").and(c.context("chromium"))
            }
            m = &mut mitm_fut => {
                stop_both();
                let c = chromium_fut.await;
                m.context("proxy").and(c.context("chromium"))
            }
            c = &mut chromium_fut => {
                stop_both();
                let m = mitm_fut.await;
                c.context("chromium").and(m.context("proxy"))
            }
        }
    }

    fn descriptor(&self) -> CaptureDescriptor {
        CaptureDescriptor {
            kind: CaptureKind::Hybrid,
            label: self.proxy_cfg.addr.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_listen_addr_dials_loopback() {
        assert_eq!(
            dial_addr("0.0.0.0:23410").unwrap(),
            "127.0.0.1:23410".parse().unwrap()
        );
        assert_eq!(dial_addr("[::]:1").unwrap(), "[::1]:1".parse().unwrap());
        assert_eq!(
            dial_addr("127.0.0.1:23410").unwrap(),
            "127.0.0.1:23410".parse().unwrap()
        );
    }
}
