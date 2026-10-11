use super::*;

// Service and container startup must not leak tokens to persistent logs.
#[test]
fn quiet_startup_cannot_be_combined_with_print_token() {
    assert!(config(&["--quiet"]).unwrap().quiet);
    assert!(!config(&[]).unwrap().quiet);
    assert!(config(&["--quiet", "--print-token"]).is_err());
}

fn config(args: &[&str]) -> Result<Config, String> {
    Config::parse(args.iter().map(std::ffi::OsString::from))
}

#[test]
fn remote_plaintext_and_incomplete_tls_configuration_are_rejected() {
    assert!(config(&["--listen", "0.0.0.0:443"]).is_err());
    assert!(config(&["--listen", "0.0.0.0:443", "--allow-remote"]).is_err());
    assert!(config(&["--tls-cert", "certificate.pem"]).is_err());
    assert!(config(&[
        "--listen",
        "0.0.0.0:443",
        "--tls-cert",
        "cert",
        "--tls-key",
        "key"
    ])
    .is_err());
    assert!(config(&[
        "--listen",
        "0.0.0.0:443",
        "--allow-remote",
        "--tls-cert",
        "cert",
        "--tls-key",
        "key",
        "--public-origin",
        "https://kubecove.example"
    ])
    .is_ok());
}

#[test]
fn unknown_flags_invalid_grace_and_invalid_origins_fail_closed() {
    assert!(config(&["--unknown"]).is_err());
    assert!(config(&["--session-grace", "-1"]).is_err());
    assert!(config(&["--session-grace", "18446744073709551615h"]).is_err());
    for origin in [
        "https://host/path",
        "http://user@host",
        "ftp://host",
        "http://host?token=secret",
    ] {
        assert!(config(&["--public-origin", origin]).is_err());
    }
}

#[test]
fn default_is_loopback_with_the_owner_selected_grace() {
    let defaults = config(&[]).unwrap();
    assert!(defaults.listen.ip().is_loopback());
    assert!(defaults.access.is_none());
    assert_eq!(defaults.grace, Duration::from_mins(5));
    assert_eq!(
        config(&["--session-grace", "0"]).unwrap().grace,
        Duration::ZERO
    );
}

#[test]
fn access_requires_a_complete_explicit_tls_configuration() {
    let flags = [
        "--cloudflare-access-team",
        "timpan",
        "--cloudflare-access-aud",
        "application-aud",
    ];
    assert!(config(&flags).is_err());
    assert!(config(&flags[..2]).is_err());
    assert!(config(&flags[2..]).is_err());
    let mut complete = flags.to_vec();
    complete.extend([
        "--tls-cert",
        "cert",
        "--tls-key",
        "key",
        "--public-origin",
        "https://kubecove.timpan.dev",
    ]);
    assert!(config(&complete).unwrap().access.is_some());
    complete.push("--print-token");
    assert!(config(&complete).is_err());
    for team in [
        "",
        "../evil",
        "evil.example",
        "https://evil",
        "-test",
        "test-",
    ] {
        assert!(CloudflareAccess::new(team, "application-aud").is_err());
    }
}

#[test]
fn oidc_requires_one_complete_https_mode_with_an_operator_login_deadline() {
    let flags = [
        "--oidc-issuer",
        "https://identity.example/realm",
        "--oidc-client-id",
        "kubecove",
        "--oidc-login-timeout",
        "5m",
    ];
    assert!(config(&flags).is_err());
    let mut complete = flags.to_vec();
    complete.extend([
        "--tls-cert",
        "cert",
        "--tls-key",
        "key",
        "--public-origin",
        "https://kubecove.example",
    ]);
    assert!(config(&complete).unwrap().oidc.is_some());
    for tail in [
        vec!["--print-token"],
        vec![
            "--cloudflare-access-team",
            "timpan",
            "--cloudflare-access-aud",
            "app",
        ],
    ] {
        let mut mixed = complete.clone();
        mixed.extend(tail);
        assert!(config(&mixed).is_err());
    }
    assert!(config(&["--oidc-client-id", "kubecove"]).is_err());
    assert!(config(&["--oidc-client-secret-file", "secret"]).is_err());
    assert!(config(&["--oidc-login-timeout", "5m"]).is_err());
    for issuer in [
        "http://identity.example",
        "https://user:pass@identity.example",
        "https://identity.example?query",
        "https://identity.example/#fragment",
    ] {
        let mut invalid = complete.clone();
        invalid[1] = issuer;
        assert!(config(&invalid).is_err());
    }
    let mut zero = complete;
    zero[5] = "0";
    assert!(config(&zero).is_err());
}
