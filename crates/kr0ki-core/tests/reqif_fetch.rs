//! Security-critical rejection paths for `reqif_fetch::fetch_reqif_url` --
//! these need no network access at all, since the scheme check and the
//! DNS-range check both happen before any connection is opened. See
//! `crates/kr0ki-core/src/reqif_fetch.rs`'s own module doc and
//! docs/superpowers/specs/2026-09-22-reqif-https-fetch-design.md section 6
//! for why the happy-path tests are separate, #[ignore]d, live tests.

use kr0ki_core::reqif_fetch::{fetch_reqif_url, FetchConfig, FetchError};

#[tokio::test]
async fn non_https_scheme_is_rejected_before_resolution() {
    let result = fetch_reqif_url("http://example.com/doc.reqif", &FetchConfig::default()).await;
    assert!(matches!(result, Err(FetchError::UnsupportedScheme { scheme }) if scheme == "http"));
}

#[tokio::test]
async fn loopback_literal_is_rejected() {
    let result = fetch_reqif_url("https://127.0.0.1/doc.reqif", &FetchConfig::default()).await;
    assert!(matches!(result, Err(FetchError::DisallowedAddress(_))));
}

#[tokio::test]
async fn cloud_metadata_endpoint_is_rejected() {
    let result = fetch_reqif_url(
        "https://169.254.169.254/latest/meta-data/",
        &FetchConfig::default(),
    )
    .await;
    assert!(matches!(result, Err(FetchError::DisallowedAddress(_))));
}

#[tokio::test]
async fn private_ipv6_literal_is_rejected() {
    let result = fetch_reqif_url("https://[fc00::1]/doc.reqif", &FetchConfig::default()).await;
    assert!(matches!(result, Err(FetchError::DisallowedAddress(_))));
}

// -- Live tests below require real network access to public HTTPS
// endpoints; #[ignore]d, matching this repo's existing live-test
// convention (see kr0ki-sysmlv2-client/tests/live.rs).

#[tokio::test]
#[ignore = "requires network access to httpbin.org"]
async fn redirect_chain_is_followed_and_revalidated() {
    let result = fetch_reqif_url("https://httpbin.org/redirect/2", &FetchConfig::default()).await;
    assert!(
        result.is_ok(),
        "expected redirect chain to resolve: {result:?}"
    );
}

#[tokio::test]
#[ignore = "requires network access to httpbin.org"]
async fn oversized_response_is_aborted_mid_stream() {
    let config = FetchConfig {
        max_bytes: 1024,
        ..FetchConfig::default()
    };
    let result = fetch_reqif_url("https://httpbin.org/bytes/1048576", &config).await;
    assert!(matches!(
        result,
        Err(FetchError::TooLarge { maximum: 1024 })
    ));
}
