use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match kortex_cli::execute(&args, &mut std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("erreur : {msg}");
            ExitCode::FAILURE
        }
    }
}
