use std::process::ExitCode;

fn main() -> ExitCode {
    println!("Rusty Buggy Language v{}", env!("CARGO_PKG_VERSION"));
    println!("The language runtime is not implemented yet.");

    ExitCode::SUCCESS
}
