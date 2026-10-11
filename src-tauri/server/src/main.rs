use kubecove_server::{
    config::{Config, HELP},
    files::write_startup_token,
    router,
    security::random_token,
    sessions::Sessions,
    AppState,
};
use std::{io, net::SocketAddr, sync::Arc};

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help") {
        println!("{HELP}");
        return Ok(());
    }
    let mut config = Config::parse(args).map_err(io::Error::other)?;
    let oidc = match config.oidc.take() {
        Some(oidc) => Some(Arc::new(
            kubecove_server::oidc::Oidc::discover(
                oidc,
                config
                    .public_origin
                    .as_deref()
                    .expect("validated OIDC origin"),
            )
            .await
            .map_err(io::Error::other)?,
        )),
        None => None,
    };
    if let Some(oidc) = oidc.as_ref().filter(|oidc| oidc.forwards_credentials()) {
        kubecove_lib::commands::enable_forwarded_bearer();
        let oidc = Arc::clone(oidc);
        tokio::spawn(async move { oidc.refresh_forever().await });
    }
    let login_mode = if oidc.is_some() {
        "OIDC"
    } else {
        "Cloudflare Access"
    };
    let token = if config.access.is_some() || oidc.is_some() {
        String::new()
    } else {
        let token = random_token()?;
        write_startup_token(&config.data_dir, &token)?;
        token
    };
    let access = config.access.take().map(Arc::new);
    kubecove_lib::commands::init_kubeconfig_settings_path(config.data_dir.clone());
    let credential_store =
        kubecove_lib::commands::initialize_server_credential_store(config.data_dir.clone())
            .map_err(io::Error::other)?
            .kind;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let sessions = Arc::new(Sessions::new(config.grace));
    if let Some(path) = &config.socket {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let listener = tokio::net::UnixListener::bind(path)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            let address = SocketAddr::from(([127, 0, 0, 1], 0));
            let policy = config.policy(address);
            let origin = config.public_origin.as_deref().ok_or_else(|| {
                io::Error::other(
                    "--socket requires --public-origin for the browser's forwarded TCP origin",
                )
            })?;
            if !config.quiet {
                print_login(origin, &token, config.print_token, login_mode);
            }
            let state = Arc::new(AppState {
                token,
                policy,
                sessions: Arc::clone(&sessions),
                credential_store,
                access,
                oidc,
            });
            axum::serve(listener, router(state))
                .with_graceful_shutdown(shutdown(Arc::clone(&sessions)))
                .await?;
            sessions.shutdown().await;
            std::fs::remove_file(path)?;
        }
        #[cfg(not(unix))]
        return Err(io::Error::other("Unix socket unsupported"));
    } else {
        let listener = std::net::TcpListener::bind(config.listen)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let policy = config.policy(address);
        let origin = config
            .public_origin
            .clone()
            .unwrap_or_else(|| policy.origins[0].clone());
        if !config.quiet {
            print_login(&origin, &token, config.print_token, login_mode);
        }
        let state = Arc::new(AppState {
            token,
            policy,
            sessions: Arc::clone(&sessions),
            credential_store,
            access,
            oidc,
        });
        if let (Some(cert), Some(key)) = (&config.tls_cert, &config.tls_key) {
            let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
            let handle = axum_server::Handle::new();
            let stopping = handle.clone();
            let stopping_sessions = Arc::clone(&sessions);
            tokio::spawn(async move {
                shutdown(stopping_sessions).await;
                stopping.shutdown();
            });
            axum_server::from_tcp_rustls(listener, tls)?
                .handle(handle)
                .serve(router(state).into_make_service())
                .await?;
        } else {
            axum::serve(tokio::net::TcpListener::from_std(listener)?, router(state))
                .with_graceful_shutdown(shutdown(Arc::clone(&sessions)))
                .await?;
        }
        sessions.shutdown().await;
    }
    Ok(())
}

fn print_login(origin: &str, token: &str, print_token: bool, login_mode: &str) {
    if token.is_empty() {
        println!("KubeCove login: {origin}/ ({login_mode})");
        return;
    }
    println!("KubeCove login: {origin}/#token={token}");
    if print_token {
        println!("{token}");
    }
}

async fn shutdown(sessions: Arc<Sessions>) {
    #[cfg(unix)]
    {
        let terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate());
        match terminate {
            Ok(mut terminate) => tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            },
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
    sessions.shutdown().await;
}
