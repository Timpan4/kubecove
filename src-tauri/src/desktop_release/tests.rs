use super::*;
use serde_json::json;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn both_transition_clients_offer_the_desktop_page_only_for_the_new_product() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let build =
        json!({"signature":"signed", "url":"https://example.test/kubecove_0.16.0_x86_64.AppImage"});
    let manifest = json!({"version":"0.16.0","platforms":{"linux-x86_64":build}});
    let mut wrong_id = manifest.clone();
    wrong_id["platforms"]["linux-x86_64"]["bundleId"] = "com.timpan.kubecove.gpui".into();
    let mut wrong_asset = manifest.clone();
    wrong_asset["platforms"]["linux-x86_64"]["url"] =
        "https://example.test/kubecove-gpui_0.16.0_x86_64.AppImage".into();
    for (status, body, expected) in [
        ("404 Not Found", Value::Null, Ok(None)),
        (
            "200 OK",
            json!({"version":"0.16.0","platforms":{}}),
            Ok(None),
        ),
        ("200 OK", manifest, Ok(Some("0.16.0".into()))),
        ("200 OK", wrong_id, Err(())),
        ("200 OK", wrong_asset, Err(())),
        ("503 Service Unavailable", Value::Null, Err(())),
    ] {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let endpoint = format!("http://{}/latest-gpui.json", listener.local_addr().unwrap());
        let server = async {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = String::new();
            tokio::io::AsyncBufReadExt::read_line(
                &mut tokio::io::BufReader::new(&mut socket),
                &mut request,
            )
            .await
            .unwrap();
            assert_eq!(request, "GET /latest-desktop.json HTTP/1.1\r\n");
            let body = body.to_string();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
        };
        let client = Client::new();
        let ((), result) = tokio::join!(server, consolidated(&client, &endpoint, "linux-x86_64"));
        assert_eq!(result.map_err(|_| ()), expected);
    }
}
