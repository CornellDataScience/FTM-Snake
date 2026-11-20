//! Parse command-line options, then hand off to the selected server runtime.
use ftm_snake::server::{self, ServerOptions};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", ServerOptions::HELP);
        return ExitCode::SUCCESS;
    }
    let options = match ServerOptions::from_args(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\nRun with --help for usage.");
            return ExitCode::from(2);
        }
    };
    let address =
        std::env::var("SNAKE_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    match server::run(options, &address).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
