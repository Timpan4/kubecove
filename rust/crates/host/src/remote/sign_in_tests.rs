// Failure mode: a computer without IPv4 loopback cannot sign in.
use super::*;

#[tokio::test]
async fn the_listener_falls_back_to_ipv6_loopback() {
    if TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).await.is_err() {
        eprintln!("skipped: this computer has no IPv6 loopback");
        return;
    }
    // A taken IPv4 port stands in for unavailable IPv4 loopback.
    let taken = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let candidates = [(taken.local_addr().unwrap(), "127.0.0.1"), LOOPBACK[1]];
    let prepared = prepare("https://kube.example", &candidates).await.unwrap();
    let link = reqwest::Url::parse(&prepared.link).unwrap();
    let redirect = link
        .query_pairs()
        .find(|(key, _)| key == "redirect_uri")
        .unwrap()
        .1
        .into_owned();
    assert!(redirect.starts_with("http://[::1]:"), "{redirect}");
    assert!(redirect.ends_with("/kubecove/callback"));
    assert!(
        link.as_str()
            .starts_with("https://kube.example/api/v1/oidc/native/start?")
    );
}
