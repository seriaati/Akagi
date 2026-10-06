//! All integration tests, linked into one binary. Each `tests/*.rs` would
//! otherwise be its own executable carrying a full copy of the crate and
//! its dependencies — hundreds of MB apiece in a debug build.

mod common;

mod analysis_bench;
mod analysis_pipeline;
mod bot_lifecycle;
mod example_bot;
mod pr_build_workflow;
mod proxy_cert_report_rewrite;
mod proxy_http_capture;
mod proxy_http_capture_all;
mod proxy_http_capture_pairing;
mod proxy_loopback_connect;
mod proxy_telemetry_block;
mod tenhou_reconnect;
