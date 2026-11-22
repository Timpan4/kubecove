use super::*;
use semver::Version;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

const PUBLISHED: (&str, &str) = (
    "200 OK",
    r#"{"version":"0.16.0","platforms":{"linux-x86_64":{"signature":"signed","url":"https://example.test/kubecove_0.16.0_x86_64.AppImage"}}}"#,
);

async fn check(
    current: Version,
    response: (&'static str, &'static str),
) -> Result<UpdateStatus, AppError> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let endpoint = format!("http://{address}/latest-desktop.json");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = String::new();
        BufReader::new(&stream).read_line(&mut request).unwrap();
        assert_eq!(request, "GET /latest-desktop.json HTTP/1.1\r\n");
        let (status, body) = response;
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let result = check_feed(&client().unwrap(), &endpoint, "linux-x86_64", &current).await;
    drop(TcpStream::connect(address));
    server.join().unwrap();
    result
}

#[tokio::test]
async fn reads_consolidated_feed_and_handles_absent_or_failing_responses() {
    let current = Version::new(0, 15, 1);
    assert!(matches!(
        check(current.clone(), PUBLISHED).await.unwrap(),
        UpdateStatus::Available(update) if update.version == "0.16.0"
    ));
    assert!(matches!(
        check(current.clone(), ("404 Not Found", "")).await.unwrap(),
        UpdateStatus::Unpublished
    ));
    assert!(matches!(
        check(current.clone(), ("204 No Content", "")).await.unwrap(),
        UpdateStatus::UpToDate
    ));
    for failed in [("503 Unavailable", ""), ("200 OK", "{")] {
        assert!(check(current.clone(), failed).await.is_err());
    }
}

#[tokio::test]
async fn only_announces_a_newer_consolidated_release() {
    for current in [Version::new(0, 16, 0), Version::new(0, 17, 0)] {
        assert!(matches!(
            check(current, PUBLISHED).await.unwrap(),
            UpdateStatus::UpToDate
        ));
    }
}
