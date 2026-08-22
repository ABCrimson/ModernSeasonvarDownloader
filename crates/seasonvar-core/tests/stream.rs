//! `Client::probe` / `Client::get_stream` against a fake CDN (wiremock) and a raw slow server.
use std::time::Duration;

use futures::StreamExt;
use seasonvar_core::test_support::mount_cdn;
use seasonvar_core::{Client, ClientConfig, CoreError, Proxy};
use url::Url;
use wiremock::MockServer;

fn client(server: &MockServer) -> Client {
    Client::new(ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        proxy: Proxy::None,
        retries: 0,
        ..ClientConfig::default()
    })
    .unwrap()
}

/// A one-shot HTTP/1.1 server on a std thread: answers the first request with `200`,
/// `Content-Length` of all chunks, then writes each chunk after its delay. Lets a test stall a
/// body mid-stream or outlive a total request deadline — wiremock can only delay whole responses.
fn slow_server(chunks: Vec<(Vec<u8>, Duration)>) -> Url {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!(
        "http://{}/slow.mp4",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else {
            return;
        };
        sock.set_nodelay(true).ok();
        let mut req = Vec::new();
        let mut buf = [0u8; 4096];
        while !req.windows(4).any(|w| w == b"\r\n\r\n") {
            match sock.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => req.extend_from_slice(&buf[..n]),
            }
        }
        let total: usize = chunks.iter().map(|(c, _)| c.len()).sum();
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nContent-Type: video/mp4\r\nConnection: close\r\n\r\n"
        );
        if sock.write_all(head.as_bytes()).is_err() {
            return;
        }
        for (chunk, delay) in chunks {
            std::thread::sleep(delay);
            if sock.write_all(&chunk).and_then(|()| sock.flush()).is_err() {
                return;
            }
        }
    });
    url
}

#[tokio::test]
async fn probe_reports_total_and_range_support() {
    let server = MockServer::start().await;
    let body: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    let url = mount_cdn(&server, "/fi2lm/x/ep1.mp4", body.clone(), true).await;
    let c = client(&server);
    let p = c.probe(&url).await.unwrap();
    assert_eq!(p.total, Some(100_000));
    assert!(p.accept_ranges);
    assert_eq!(p.etag.as_deref(), Some("\"etag-100000\""));
    let url2 = mount_cdn(&server, "/plain.mp4", body, false).await;
    let p2 = c.probe(&url2).await.unwrap();
    assert_eq!(p2.total, Some(100_000));
    assert!(!p2.accept_ranges);
}

#[tokio::test]
async fn get_stream_delivers_exact_ranges() {
    let server = MockServer::start().await;
    let body: Vec<u8> = (0..50_000u32).map(|i| (i % 199) as u8).collect();
    let url = mount_cdn(&server, "/ep.mp4", body.clone(), true).await;
    let c = client(&server);
    let mut s = c
        .get_stream(&url, Some((10_000, Some(19_999))), Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(s.status, 206);
    assert_eq!(s.content_length, Some(10_000));
    let mut got = Vec::new();
    while let Some(chunk) = s.body.next().await {
        got.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(got, body[10_000..20_000].to_vec());
    // open-ended tail
    let mut s = c
        .get_stream(&url, Some((49_990, None)), Duration::from_secs(5))
        .await
        .unwrap();
    let mut got = Vec::new();
    while let Some(chunk) = s.body.next().await {
        got.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(got, body[49_990..].to_vec());
}

#[tokio::test]
async fn get_stream_rejects_servers_that_ignore_range() {
    let server = MockServer::start().await;
    let url = mount_cdn(&server, "/norange.mp4", vec![7u8; 1000], false).await;
    let c = client(&server);
    let err = c
        .get_stream(&url, Some((10, Some(20))), Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Protocol(_)), "{err:?}");
    let full = c
        .get_stream(&url, None, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(full.status, 200);
}

#[tokio::test]
async fn get_stream_maps_404_to_http_error() {
    let server = MockServer::start().await;
    let c = client(&server);
    let url = Url::parse(&format!("{}/missing.mp4", server.uri())).unwrap();
    let err = c
        .get_stream(&url, None, Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::Http { status: 404, .. }),
        "{err:?}"
    );
}

/// A media body streams for minutes: `ClientConfig::timeout` (the total deadline for site
/// requests) must not cut a `get_stream` body short — only `read_timeout` (per chunk) bounds it.
#[tokio::test]
async fn get_stream_outlives_the_client_total_timeout() {
    let url = slow_server(vec![
        (b"hello".to_vec(), Duration::ZERO),
        (b"world".to_vec(), Duration::from_millis(600)),
    ]);
    let c = Client::new(ClientConfig {
        proxy: Proxy::None,
        retries: 0,
        timeout: Duration::from_millis(200),
        ..ClientConfig::default()
    })
    .unwrap();
    let mut s = c
        .get_stream(&url, None, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(s.content_length, Some(10));
    let mut got = Vec::new();
    while let Some(chunk) = s.body.next().await {
        got.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(got, b"helloworld");
}

#[tokio::test]
async fn get_stream_times_out_when_the_body_stalls() {
    let url = slow_server(vec![
        (b"hello".to_vec(), Duration::ZERO),
        (b"world".to_vec(), Duration::from_secs(5)),
    ]);
    let c = Client::new(ClientConfig {
        proxy: Proxy::None,
        retries: 0,
        ..ClientConfig::default()
    })
    .unwrap();
    let mut s = c
        .get_stream(&url, None, Duration::from_secs(1))
        .await
        .unwrap();
    let mut got = Vec::new();
    let err = loop {
        match s.body.next().await {
            Some(Ok(chunk)) => got.extend_from_slice(&chunk),
            Some(Err(e)) => break e,
            None => panic!("stream ended without a timeout error"),
        }
    };
    assert_eq!(got, b"hello");
    assert!(matches!(err, CoreError::Timeout(_)), "{err:?}");
    assert_eq!(err.kind(), "timeout");
    assert!(err.hint().unwrap().contains("stalled"));
    // the stream is fused after the timeout
    assert!(s.body.next().await.is_none());
}
