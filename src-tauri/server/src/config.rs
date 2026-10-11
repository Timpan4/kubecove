use crate::{access::CloudflareAccess, oidc::OidcConfig, security::NetworkPolicy};
use axum::http::Uri;
use std::{ffi::OsString, net::SocketAddr, path::PathBuf, time::Duration};

pub const HELP: &str = "kubecove-server [OPTIONS]\n\
  --listen <IP:PORT>       Loopback by default; port 0 chooses a free port\n\
  --socket <PATH>          Unix socket instead of TCP\n\
  --data-dir <PATH>        Settings, startup token, and credentials directory\n\
  --session-grace <TIME>   Reconnect grace, default 5m; 0 stops on disconnect\n\
  --allow-remote          Required for non-loopback TCP\n\
  --tls-cert <PATH>       PEM certificate chain\n\
  --tls-key <PATH>        PEM private key\n\
  --public-origin <URL>   Exact browser origin, including an SSH tunnel's port\n\
  --cloudflare-access-team <NAME>  Use verified Cloudflare Access sign-in\n\
  --cloudflare-access-aud <AUD>    Exact Access application audience tag\n\
  --oidc-issuer <URL>     HTTPS OpenID Connect issuer\n\
  --oidc-client-id <ID>   Registered OIDC application client ID\n\
  --oidc-client-secret-file <PATH>  Optional confidential-client secret\n\
  --oidc-login-timeout <TIME>  Required OIDC transaction and network deadline\n\
  --print-token          Also print the token on startup\n\
  --quiet                Suppress the startup login URL in service logs\n\
  --help                 Show this help";

pub struct Config {
    pub listen: SocketAddr,
    pub socket: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub grace: Duration,
    pub allow_remote: bool,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    pub public_origin: Option<String>,
    pub print_token: bool,
    pub quiet: bool,
    pub access: Option<CloudflareAccess>,
    pub oidc: Option<OidcConfig>,
}

impl Config {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut config = Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 0)),
            socket: None,
            data_dir: PathBuf::new(),
            // Product-owner choice recorded in ADR 0020.
            grace: Duration::from_mins(5),
            allow_remote: false,
            tls_cert: None,
            tls_key: None,
            public_origin: None,
            print_token: false,
            quiet: false,
            access: None,
            oidc: None,
        };
        let mut access_team = None;
        let mut access_aud = None;
        let mut oidc_issuer = None;
        let mut oidc_client = None;
        let mut oidc_secret = None;
        let mut oidc_timeout = None;
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let flag = flag.to_str().ok_or("option names must be UTF-8")?;
            match flag {
                "--allow-remote" => config.allow_remote = true,
                "--print-token" => config.print_token = true,
                "--quiet" => config.quiet = true,
                "--listen" => {
                    config.listen = text(value(&mut args, flag)?)?
                        .parse()
                        .map_err(|_| "--listen requires IP:PORT")?;
                }
                "--socket" => config.socket = Some(value(&mut args, flag)?.into()),
                "--data-dir" => config.data_dir = value(&mut args, flag)?.into(),
                "--tls-cert" => config.tls_cert = Some(value(&mut args, flag)?.into()),
                "--tls-key" => config.tls_key = Some(value(&mut args, flag)?.into()),
                "--public-origin" => config.public_origin = Some(text(value(&mut args, flag)?)?),
                "--cloudflare-access-team" => access_team = Some(text(value(&mut args, flag)?)?),
                "--cloudflare-access-aud" => access_aud = Some(text(value(&mut args, flag)?)?),
                "--oidc-issuer" => oidc_issuer = Some(text(value(&mut args, flag)?)?),
                "--oidc-client-id" => oidc_client = Some(text(value(&mut args, flag)?)?),
                "--oidc-client-secret-file" => oidc_secret = Some(value(&mut args, flag)?.into()),
                "--oidc-login-timeout" => {
                    oidc_timeout = Some(duration(&text(value(&mut args, flag)?)?, flag)?);
                }
                "--session-grace" => {
                    config.grace = duration(&text(value(&mut args, flag)?)?, flag)?;
                }
                _ => return Err(format!("unknown option: {flag}")),
            }
        }
        config.access = match (access_team, access_aud) {
            (None, None) => None,
            (Some(team), Some(aud)) => Some(CloudflareAccess::new(&team, &aud)?),
            _ => return Err(
                "--cloudflare-access-team and --cloudflare-access-aud must be supplied together"
                    .into(),
            ),
        };
        config.oidc = match (oidc_issuer, oidc_client, oidc_timeout, oidc_secret) {
            (None, None, None, None) => None,
            (Some(issuer), Some(client), Some(timeout), secret) => {
                Some(OidcConfig::new(issuer, client, secret, timeout)?)
            }
            _ => {
                return Err(
                    "OIDC requires --oidc-issuer, --oidc-client-id and --oidc-login-timeout".into(),
                )
            }
        };
        if config.data_dir.as_os_str().is_empty() {
            config.data_dir = default_data_dir()?;
        }
        config.validate()?;
        Ok(config)
    }

    fn validate(&mut self) -> Result<(), String> {
        if self.quiet && self.print_token {
            return Err("--quiet cannot be combined with --print-token".into());
        }
        if self.oidc.is_some() && self.access.is_some() {
            return Err(
                "OIDC and Cloudflare Access are mutually exclusive authentication modes".into(),
            );
        }
        if self.oidc.is_some()
            && (self.tls_cert.is_none() || self.public_origin.is_none() || self.print_token)
        {
            return Err(
                "OIDC mode requires TLS and --public-origin, and cannot use --print-token".into(),
            );
        }
        if self.access.is_some()
            && (self.tls_cert.is_none() || self.public_origin.is_none() || self.print_token)
        {
            return Err("Cloudflare Access mode requires TLS and --public-origin, and cannot use --print-token".into());
        }
        if self.tls_cert.is_some() != self.tls_key.is_some() {
            return Err("--tls-cert and --tls-key must be supplied together".into());
        }
        if self.socket.is_none()
            && !self.listen.ip().is_loopback()
            && (!self.allow_remote || self.tls_cert.is_none())
        {
            return Err(
                "non-loopback listeners require --allow-remote, --tls-cert, and --tls-key".into(),
            );
        }
        if self.socket.is_some() && (self.allow_remote || self.tls_cert.is_some()) {
            return Err("Unix sockets cannot be combined with remote TCP or TLS options".into());
        }
        #[cfg(not(unix))]
        if self.socket.is_some() {
            return Err("Unix sockets are unavailable on this platform".into());
        }
        if let Some(origin) = self.public_origin.as_mut() {
            let uri: Uri = origin.parse().map_err(|_| "invalid --public-origin")?;
            let scheme = if self.tls_cert.is_some() {
                "https"
            } else {
                "http"
            };
            let authority = uri.authority().ok_or("--public-origin requires a host")?;
            if uri.scheme_str() != Some(scheme)
                || !authority
                    .as_str()
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".-:[]".contains(&byte))
                || uri
                    .path_and_query()
                    .is_some_and(|path| path.as_str() != "/")
            {
                return Err("--public-origin must be an exact HTTP(S) origin matching the listener's TLS mode".into());
            }
            *origin = format!("{scheme}://{authority}");
        }
        if self.socket.is_some() && self.public_origin.is_none() {
            return Err(
                "--socket requires --public-origin for the browser's forwarded TCP origin".into(),
            );
        }
        Ok(())
    }

    pub fn policy(&self, address: SocketAddr) -> NetworkPolicy {
        let scheme = if self.tls_cert.is_some() {
            "https"
        } else {
            "http"
        };
        let mut hosts = vec![address.to_string()];
        if address.ip().is_loopback() {
            hosts.extend([
                format!("localhost:{}", address.port()),
                format!("127.0.0.1:{}", address.port()),
                format!("[::1]:{}", address.port()),
            ]);
        }
        let mut origins = hosts
            .iter()
            .map(|host| format!("{scheme}://{host}"))
            .collect::<Vec<_>>();
        if let Some(origin) = &self.public_origin {
            let uri: Uri = origin.parse().expect("validated origin");
            hosts.push(uri.authority().expect("validated host").to_string());
            origins.push(origin.clone());
        }
        NetworkPolicy {
            hosts,
            origins,
            secure: self.tls_cert.is_some(),
        }
    }
}

fn value(args: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<OsString, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn text(value: OsString) -> Result<String, String> {
    value
        .into_string()
        .map_err(|_| "option value must be UTF-8".into())
}

fn duration(value: &str, flag: &str) -> Result<Duration, String> {
    let (number, multiplier) = if let Some(value) = value.strip_suffix('m') {
        (value, 60)
    } else if let Some(value) = value.strip_suffix('h') {
        (value, 60 * 60)
    } else {
        (value.strip_suffix('s').unwrap_or(value), 1)
    };
    let seconds = number
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(multiplier))
        .ok_or_else(|| {
            format!("{flag} requires non-negative seconds, minutes (m), or hours (h)")
        })?;
    // Tokio's Instant must be able to represent the configured deadline.
    let duration = Duration::from_secs(seconds);
    std::time::Instant::now()
        .checked_add(duration)
        .ok_or_else(|| format!("{flag} exceeds the platform clock range"))?;
    Ok(duration)
}

fn default_data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
    });
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    base.map(|base| base.join("com.timpan.kubecove"))
        .ok_or_else(|| "platform config directory is unavailable; supply --data-dir".into())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
