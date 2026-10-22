//! Portable Cargo workspace wrapper. Windows cannot execute a POSIX .sh wrapper.
use std::{
    env,
    process::{Command, ExitCode},
};
fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let Some(compiler) = args.next() else {
        eprintln!("coverage wrapper requires the compiler path");
        return ExitCode::FAILURE;
    };
    let mut command = Command::new(compiler);
    command.args(args).args(["-C", "instrument-coverage"]);
    // Continuous counters must align to Apple Silicon's page size, including on Intel.
    if cfg!(target_os = "macos") {
        for section in ["__llvm_prf_cnts", "__llvm_prf_data", "__llvm_prf_bits"] {
            command.args([
                "-C",
                &format!("link-arg=-Wl,-sectalign,__DATA,{section},0x4000"),
            ]);
        }
    }
    if env::var("LOTSE_COVERAGE_BRANCH").as_deref() == Ok("1") {
        command.args(["-Z", "coverage-options=branch"]);
    }
    match command.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("cannot start instrumented compiler: {error}");
            ExitCode::FAILURE
        }
    }
}
