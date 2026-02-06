use clap::{Parser, Subcommand};
use sr_common::{ErrorItem, SR_CMP_002};
use sr_compiler::compile_dry_run;
use sr_policy::{load_policy_from_path, validate_policy};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "safe-run")]
#[command(about = "Safe-Run M0 CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Validate {
        policy: String,
    },
    Compile {
        #[arg(long = "dry-run", default_value_t = false)]
        dry_run: bool,
        #[arg(long)]
        policy: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { policy } => validate_cmd(&policy),
        Commands::Compile { dry_run, policy } => compile_cmd(dry_run, &policy),
    }
}

fn validate_cmd(policy_path: &str) -> ExitCode {
    match load_policy_from_path(policy_path) {
        Ok(policy) => {
            let result = validate_policy(policy);
            print_json_value(&serde_json::to_value(&result).expect("convert validation result"));
            if result.valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(err) => {
            print_error_result(&err);
            ExitCode::from(2)
        }
    }
}

fn compile_cmd(dry_run: bool, policy_path: &str) -> ExitCode {
    if !dry_run {
        let err = ErrorItem::new(SR_CMP_002, "compile.dryRun", "M0 only supports --dry-run");
        print_error_result(&err);
        return ExitCode::from(2);
    }

    let policy = match load_policy_from_path(policy_path) {
        Ok(policy) => policy,
        Err(err) => {
            print_error_result(&err);
            return ExitCode::from(2);
        }
    };

    let validation = validate_policy(policy);
    if !validation.valid {
        print_json_value(&serde_json::to_value(&validation).expect("convert validation result"));
        return ExitCode::from(2);
    }

    let normalized = validation
        .normalized_policy
        .expect("normalized policy exists on valid result");

    match compile_dry_run(&normalized) {
        Ok(bundle) => {
            print_json_value(&serde_json::to_value(&bundle).expect("convert compile bundle"));
            ExitCode::SUCCESS
        }
        Err(err) => {
            print_error_result(&err);
            ExitCode::from(2)
        }
    }
}

fn print_json_value(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serialize json output")
    );
}

fn print_error_result(err: &ErrorItem) {
    print_json_value(&serde_json::json!({
        "valid": false,
        "errors": [err],
        "warnings": [],
        "normalizedPolicy": null
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_requires_dry_run_flag() {
        let code = compile_cmd(false, "unused.yaml");
        assert_eq!(code, ExitCode::from(2));
    }
}
