#[tokio::main]
async fn main() -> std::process::ExitCode {
    let service = normalize_facts::service::FactsCliService::new();
    match service.cli_run_async().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e);
            std::process::ExitCode::FAILURE
        }
    }
}
