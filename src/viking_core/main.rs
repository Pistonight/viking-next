use std::env::Args;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args();
    let Some(arg0) = args.next() else {
        print_multicall_help();
        return ExitCode::FAILURE;
    };
    let arg0 = PathBuf::from(arg0);
    let Some(exe_name) = arg0.file_stem() else {
        print_multicall_help();
        return ExitCode::FAILURE;
    };
    match run_with_exe_name(exe_name.as_encoded_bytes(), args, true) {
        Err(e) => {
            eprintln!("fatal: {e:?}");
            ExitCode::FAILURE
        }
        Ok(code) => ExitCode::from(code as u8),
    }
}

fn run_with_exe_name(exe_name: &[u8], mut args: Args, multicall: bool) -> cu::Result<u32> {
    if exe_name.ends_with(b"check") {
        return viking::cli::check_argv(args.collect());
    }
    if exe_name.ends_with(b"decompme") {
        return viking::cli::decompme_argv(args.collect());
    }
    if exe_name.ends_with(b"listsym") {
        return viking::cli::listsym_argv(args.collect());
    }
    if !multicall {
        print_multicall_help();
        return Ok(127);
    }
    let Some(command) = args.next() else {
        print_multicall_help();
        return Ok(127);
    };
    run_with_exe_name(command.as_ref(), args, false)
}

fn print_multicall_help() {
    eprintln!(
        "NX decomp tools multi-call binary\n\nThe command that runs depends on the executable name, or call a command directly: viking COMMAND [ARGS...]"
    );
}
