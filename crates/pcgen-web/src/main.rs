//! `pcgen-web` entrypoint: binds a local axum server.

use axum::Router;
use pcgen_web::{app, WebConfig};
use std::net::SocketAddr;
use std::path::PathBuf;

const USAGE: &str = "\
pcgen-web — local character builder server

Usage: pcgen-web [OPTIONS]

Options:
      --data-root <DIR>         PCGen data root (default: .)
      --cohort-root <DIR>       Cohort definitions root (default: cwd)
      --characters-root <DIR>   Where character saves live (default: characters)
  -h, --help                    Print this message

Environment:
  PCGEN_WEB_ADDR   Listen address (default: 127.0.0.1:8787)
";

/// What the entrypoint should do, after reading `argv`.
enum Action {
    Serve(WebConfig),
    Help,
    Fail(String),
}

/// Read `argv`.
///
/// Unknown arguments are refused rather than ignored.  They used to be skipped
/// silently, so `pcgen-web --help` started a server instead of printing usage,
/// and a mistyped `--data-rot` started one against the current directory with
/// no warning at all — which looks like the data failed to load rather than
/// like the flag was wrong.
fn parse_args(args: impl Iterator<Item = String>) -> Action {
    let mut data_root = PathBuf::from(".");
    let mut cohort_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut characters_root = PathBuf::from("characters");

    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        let target = match arg.as_str() {
            "-h" | "--help" => return Action::Help,
            "--data-root" => &mut data_root,
            "--cohort-root" => &mut cohort_root,
            "--characters-root" => &mut characters_root,
            other => return Action::Fail(format!("unknown argument: {other}")),
        };
        match args.next() {
            Some(value) => *target = PathBuf::from(value),
            None => {
                return Action::Fail(format!("{arg} needs a directory"));
            }
        }
    }

    Action::Serve(WebConfig {
        data_root,
        cohort_root,
        characters_root,
    })
}

#[tokio::main]
async fn main() {
    let config = match parse_args(std::env::args().skip(1)) {
        Action::Serve(config) => config,
        Action::Help => {
            print!("{USAGE}");
            return;
        }
        Action::Fail(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> impl Iterator<Item = String> {
        items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn help_is_not_a_server_start() {
        assert!(matches!(parse_args(args(&["--help"])), Action::Help));
        assert!(matches!(parse_args(args(&["-h"])), Action::Help));
    }

    #[test]
    fn an_unknown_flag_is_refused() {
        // The typo that used to start a server against the wrong directory.
        match parse_args(args(&["--data-rot", "/tmp"])) {
            Action::Fail(message) => assert!(message.contains("--data-rot")),
            _ => panic!("a mistyped flag must not be ignored"),
        }
    }

    #[test]
    fn a_flag_without_a_value_is_refused() {
        match parse_args(args(&["--data-root"])) {
            Action::Fail(message) => assert!(message.contains("--data-root")),
            _ => panic!("a flag with no value must not be ignored"),
        }
    }

    #[test]
    fn paths_are_read() {
        match parse_args(args(&[
            "--data-root",
            "/data",
            "--characters-root",
            "/chars",
        ])) {
            Action::Serve(config) => {
                assert_eq!(config.data_root, PathBuf::from("/data"));
                assert_eq!(config.characters_root, PathBuf::from("/chars"));
            }
            _ => panic!("expected a serve action"),
        }
    }

    #[test]
    fn no_arguments_serves_with_defaults() {
        assert!(matches!(parse_args(args(&[])), Action::Serve(_)));
    }
}
