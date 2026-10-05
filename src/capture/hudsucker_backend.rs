//! `CaptureBackend` impl that delegates to the existing hudsucker MITM
//! proxy. Zero behaviour change from previous releases — this is just an
//! adapter so the supervisor can multiplex MITM and Chromium uniformly.

use super::{CaptureBackend, CaptureCtx, CaptureDescriptor, CaptureKind, ShutdownToken};
use crate::config::{HttpCaptureConfig, ProxyConfig};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::info;

pub struct HudsuckerBackend {
    proxy_cfg: ProxyConfig,
    http_cfg: HttpCaptureConfig,
    /// Shared with `AppState.capture_control.force_close` — `notify_waiters`
    /// kicks every in-flight WS so existing flows actually disconnect (not
    /// just drain naturally) when the supervisor stops the backend.
    pub force_close: Arc<Notify>,
    /// Hybrid mode: a CDP-controlled browser is in front of this proxy, so
    /// the bridges also feed the page-autoplay slots (time budget, input
    /// watch, Tenhou state) that the chromium backend would otherwise fill.
    page_autoplay: bool,
}

impl HudsuckerBackend {
    pub fn new(
        proxy_cfg: ProxyConfig,
        http_cfg: HttpCaptureConfig,
        force_close: Arc<Notify>,
    ) -> Self {
        Self {
            proxy_cfg,
            http_cfg,
            force_close,
            page_autoplay: false,
        }
    }

    /// See [`Self::page_autoplay`].
    pub fn with_page_autoplay(mut self) -> Self {
        self.page_autoplay = true;
        self
    }
}

#[async_trait]
impl CaptureBackend for HudsuckerBackend {
    async fn run(self: Box<Self>, ctx: CaptureCtx, shutdown: ShutdownToken) -> Result<()> {
        let addr = self.proxy_cfg.addr.clone();
        info!("hudsucker backend starting on {addr}");

        // hudsucker takes a `Future<Output = ()>` graceful-shutdown signal.
        // Bridge the supervisor's `ShutdownToken` into a oneshot-like future.
        let shutdown_fut = async move {
            shutdown.wait().await;
        };

        let hooks = ctx
            .autoplay
            .as_ref()
            .map(|a| {
                let page = self.page_autoplay;
                crate::bridge::BridgeHooks {
                    time_budget: page.then(|| a.time_budget.clone()),
                    input_watch: page.then(|| a.input_watch.clone()),
                    tenhou_state: page.then(|| a.tenhou_state.clone()),
                    // Riichi City autoplay injects frames through the relay;
                    // other platforms ignore the channel (their autoplay
                    // clicks a page).
                    riichi_inject: Some(a.inject.clone()),
                    notify: None,
                }
            })
            .unwrap_or_default();

        crate::proxy::start_proxy(
            self.proxy_cfg,
            self.http_cfg,
            ctx.platform,
            ctx.session,
            Some(ctx.mjai_bus),
            Some(ctx.notify_bus),
            self.force_close,
            hooks,
            shutdown_fut,
        )
        .await
    }

    fn descriptor(&self) -> CaptureDescriptor {
        CaptureDescriptor {
            kind: CaptureKind::Mitm,
            label: self.proxy_cfg.addr.clone(),
        }
    }
}
