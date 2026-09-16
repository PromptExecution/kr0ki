//! Live contract for the executable playb00k catalog.
//!
//! Run with `KR0KI_PLAYBOOK_URL=http://192.168.1.137:8787 just test-playbook`.
//! This intentionally exercises the HTTP surface the Vue panel uses, not
//! `HttpKrokiBackend` directly.

use kr0ki_core::examples::ALL;

#[tokio::test]
#[ignore = "requires KR0KI_PLAYBOOK_URL pointing at a running kr0ki service"]
async fn every_playbook_fixture_renders_and_caches() {
    let base = std::env::var("KR0KI_PLAYBOOK_URL")
        .expect("set KR0KI_PLAYBOOK_URL, e.g. http://192.168.1.137:8787");
    let base = base.trim_end_matches('/');
    let client = reqwest::Client::new();

    for example in ALL {
        for output in example.outputs {
            let url = format!("{base}/render/{}?output={output}", example.format);
            let first = client
                .post(&url)
                .header("Content-Type", "text/plain")
                .body(example.source)
                .send()
                .await
                .unwrap_or_else(|error| panic!("{} {output}: request failed: {error}", example.id));
            assert!(
                first.status().is_success(),
                "{} {output}: unexpected status {}",
                example.id,
                first.status()
            );
            let first_key = first
                .headers()
                .get("x-kr0ki-key")
                .expect("successful render includes cache key")
                .to_str()
                .unwrap()
                .to_owned();
            let first_bytes = first.bytes().await.unwrap();
            assert_rendered_bytes(example.id, output, &first_bytes);

            let second = client
                .post(&url)
                .header("Content-Type", "text/plain")
                .body(example.source)
                .send()
                .await
                .unwrap_or_else(|error| {
                    panic!("{} {output}: second request failed: {error}", example.id)
                });
            assert!(second.status().is_success());
            assert_eq!(
                second.headers().get("x-kr0ki-cache").unwrap(),
                "hit",
                "{} {output}: second request must be a cache hit",
                example.id
            );
            assert_eq!(
                second
                    .headers()
                    .get("x-kr0ki-key")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                first_key
            );
            assert_eq!(second.bytes().await.unwrap().as_ref(), first_bytes.as_ref());
        }
    }
}

fn assert_rendered_bytes(example: &str, output: &str, bytes: &[u8]) {
    match output {
        "svg" => assert!(
            bytes.windows(4).any(|window| window == b"<svg"),
            "{example}: expected SVG bytes"
        ),
        "png" => assert!(
            bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "{example}: expected PNG signature"
        ),
        other => panic!("{example}: unknown output {other}"),
    }
}
