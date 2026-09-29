//! `pcgen-web` entrypoint: binds a local axum server.

use axum::Router;
use pcgen_web::{app, WebConfig};
use std::net::SocketAddr;
use std::path::PathBuf;

fn config_from_args() -> WebConfig {
    let mut args = std::env::args().skip(1);
    let mut data_root = PathBuf::from(".");
    let mut cohort_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut characters_root = PathBuf::from("characters");
    while let Some(arg) = args.next() {
        if arg == "--data-root" {
            if let Some(value) = args.next() {
                data_root = PathBuf::from(value);
            }
        } else if arg == "--cohort-root" {
            if let Some(value) = args.next() {
                cohort_root = PathBuf::from(value);
            }
        } else if arg == "--characters-root" {
            if let Some(value) = args.next() {
                characters_root = PathBuf::from(value);
            }
        }
    }
    WebConfig {
        data_root,
        cohort_root,
        characters_root,
    }
}

#[tokio::main]
async fn main() {
    let config = config_from_args();
    let addr: SocketAddr = std::env::var("PCGEN_WEB_ADDR")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 8787)));
    let router: Router = app(config);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind pcgen-web listener");
    eprintln!("pcgen-web listening on http://{addr}");
    axum::serve(listener, router)
        .await
        .expect("serve pcgen-web");
}
