use axum_server::tls_rustls::RustlsConfig;
use std::net::SocketAddr;
use std::path::PathBuf;
use tgprep_server::{Client, ServeError, router};

#[derive(Debug, thiserror::Error)]
enum StartError {
    #[error("TGPREP_LISTEN is not an address: {0}")]
    Listen(#[from] std::net::AddrParseError),
    #[error("TGPREP_TLS_CERT and TGPREP_TLS_KEY go together")]
    HalfTls,
    #[error(transparent)]
    Client(#[from] ServeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn variable(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

async fn run() -> Result<(), StartError> {
    let address: SocketAddr = variable("TGPREP_LISTEN")
        .as_deref()
        .unwrap_or("127.0.0.1:5180")
        .parse()?;
    let directory = PathBuf::from(variable("TGPREP_CLIENT_DIR").unwrap_or_else(|| "dist".into()));
    let app = router(Client::load(&directory)?).into_make_service();
    match (variable("TGPREP_TLS_CERT"), variable("TGPREP_TLS_KEY")) {
        (Some(certificate), Some(key)) => {
            let _ = rustls::crypto::ring::default_provider().install_default();
            let tls = RustlsConfig::from_pem_file(certificate, key).await?;
            println!("tgprep on https://{address}");
            axum_server::bind_rustls(address, tls).serve(app).await?;
        }
        (None, None) => {
            println!("tgprep on http://{address}");
            axum_server::bind(address).serve(app).await?;
        }
        _ => return Err(StartError::HalfTls),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
