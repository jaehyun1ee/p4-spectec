use std::{
    env,
    ffi::{OsStr, OsString},
    path::PathBuf,
    process::ExitCode,
};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{parse::parse_file, unparse::P4Unparser},
    interp::al::{Al, Config, context::Global},
    lang::traits::print::Print,
    pass::{algo, elaborate},
    runner::{BuiltinInterface, NullExtern, Runner},
};

const USAGE: &str = "Usage: p4spec-rust <elab|algo> <path>...
       p4spec-rust run --al <spec-path>... --rel <relation> -p <program.p4>
           [-i <include-dir>]... [--det] [--guard]";

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let Some(command) = args.next() else {
        return usage_error();
    };

    if command == OsStr::new("--help") || command == OsStr::new("-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if command == OsStr::new("run") {
        return run_command(args.collect());
    }
    if command != OsStr::new("elab") && command != OsStr::new("algo") {
        eprintln!("unknown command: {}", command.to_string_lossy());
        return usage_error();
    }

    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    if matches!(paths.as_slice(), [path] if path == OsStr::new("--help") || path == OsStr::new("-h"))
    {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if paths.is_empty() {
        return usage_error();
    }

    let spec_el = match parse_files(paths) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let spec_il = match elaborate::elaborate(spec_el) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };

    if command == OsStr::new("elab") {
        println!("{}", Print::to_string(&spec_il));
        return ExitCode::SUCCESS;
    }

    let spec_al = match algo::convert(spec_il) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    println!("{}", Print::to_string(&spec_al));
    ExitCode::SUCCESS
}

fn usage_error() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn command_error(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}

struct RunOptions {
    paths: Vec<PathBuf>,
    relation: String,
    program: PathBuf,
    includes: Vec<PathBuf>,
    det: bool,
    guard: bool,
}

impl RunOptions {
    fn parse(args: Vec<OsString>) -> Option<Self> {
        let mut args = args.into_iter();
        let mut paths = Vec::new();
        let mut relation = None;
        let mut program = None;
        let mut includes = Vec::new();
        let mut al = false;
        let mut det = false;
        let mut guard = false;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--al") if !al => al = true,
                Some("--det") if !det => det = true,
                Some("--guard") if !guard => guard = true,
                Some("--rel") if relation.is_none() => {
                    relation = Some(option_value(&mut args)?.into_string().ok()?);
                }
                Some("-p") if program.is_none() => {
                    program = Some(PathBuf::from(option_value(&mut args)?));
                }
                Some("-i") => includes.push(PathBuf::from(option_value(&mut args)?)),
                _ if arg.to_string_lossy().starts_with('-') => return None,
                _ => paths.push(PathBuf::from(arg)),
            }
        }
        if !al || paths.is_empty() {
            return None;
        }
        Some(Self {
            paths,
            relation: relation?,
            program: program?,
            includes,
            det,
            guard,
        })
    }
}

fn option_value(args: &mut impl Iterator<Item = OsString>) -> Option<OsString> {
    args.next()
        .filter(|value| !value.is_empty() && !value.to_string_lossy().starts_with('-'))
}

fn run_command(args: Vec<OsString>) -> ExitCode {
    if matches!(args.as_slice(), [arg] if arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(options) = RunOptions::parse(args) else {
        return usage_error();
    };
    let spec_el = match parse_files(options.paths) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let spec_il = match elaborate::elaborate(spec_el) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let spec_al = match algo::convert(spec_il) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let unparser = P4Unparser::from_al_spec(&spec_al);
    let global = match Global::load(spec_al) {
        Ok(global) => global,
        Err(error) => return command_error(error),
    };
    let program = match parse_file(&options.includes, options.program) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("syntax error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut runner = Runner::<Al, _, _>::new(
        global,
        Config::new(options.det, options.guard),
        BuiltinInterface::new(unparser),
        NullExtern,
    );
    match runner.eval_program(&options.relation, program) {
        Ok(_) => {
            println!("passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("runtime error: {error}");
            ExitCode::FAILURE
        }
    }
}
