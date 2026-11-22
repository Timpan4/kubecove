use super::*;
use semver::Version;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

const LEGACY: (&str, &str) = (
    "200 OK",
    r#"{"version":"0.15.1","platforms":{"linux-x86_64":{"signature":"signed","url":"https://example.test/kubecove-gpui_0.15.1_x86_64.AppImage"}}}"#,
);

async fn check(
    current: Version,
    responses: Vec<(&'static str, &'static str)>,
) -> Result<UpdateStatus, AppError> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let endpoint = format!("http://{address}/latest-gpui.json");
    let server = std::thread::spawn(move || {
        for (path, (status, body)) in ["latest-desktop.json", "latest-gpui.json"]
            .into_iter()
            .zip(responses)
        {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(&stream).read_line(&mut request).unwrap();
            assert_eq!(request, format!("GET /{path} HTTP/1.1\r\n"));
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    let result = check_feeds(&client().unwrap(), &endpoint, "linux-x86_64", &current).await;
    // Unblock accept if the client skipped an expected request. EOF fails the request assertion.
    drop(TcpStream::connect(address));
    server.join().unwrap();
    result
}

#[tokio::test]
async fn checks_legacy_after_consolidated_failure_and_handles_absent_feeds() {
    let current = Version::new(0, 15, 0);
    let missing = ("404 Not Found", "");
    for failed in [("503 Unavailable", ""), ("200 OK", "{")] {
        assert!(matches!(
            check(current.clone(), vec![failed, LEGACY]).await.unwrap(),
            UpdateStatus::Available(_)
        ));
        assert!(check(current.clone(), vec![failed, missing]).await.is_err());
    }
    assert!(matches!(
        check(current, vec![missing, missing]).await.unwrap(),
        UpdateStatus::Unpublished
    ));
}

#[tokio::test]
async fn reports_consolidated_failure_when_legacy_is_up_to_date() {
    let error = check(
        Version::new(0, 15, 1),
        vec![("503 Unavailable", ""), LEGACY],
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, AppErrorKind::Network);
    assert_eq!(error.message, "Desktop release check failed");
}

#[tokio::test]
async fn only_announces_a_newer_consolidated_release() {
    let published = (
        "200 OK",
        r#"{"version":"0.16.0","platforms":{"linux-x86_64":{"signature":"signed","url":"https://example.test/kubecove_0.16.0_x86_64.AppImage"}}}"#,
    );
    for current in [Version::new(0, 16, 0), Version::new(0, 17, 0)] {
        assert!(matches!(
            check(current, vec![published]).await.unwrap(),
            UpdateStatus::UpToDate
        ));
    }
}
