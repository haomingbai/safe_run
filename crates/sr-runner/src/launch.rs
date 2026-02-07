use crate::constants::FIRECRACKER_CONFIG_FILE;
use crate::model::{CommandSpec, LaunchPlan, RunnerRuntime};
use sr_compiler::CompileBundle;
use std::path::Path;

/// Build jailer and Firecracker command lines based on the compile bundle.
pub(crate) fn assemble_launch_plan(
    run_id: &str,
    workdir: &Path,
    compile_bundle: &CompileBundle,
    runtime: &RunnerRuntime,
) -> LaunchPlan {
    let firecracker_args = vec![
        "--config-file".to_string(),
        workdir
            .join(FIRECRACKER_CONFIG_FILE)
            .to_string_lossy()
            .to_string(),
    ];

    let mut jailer_args = vec![
        "--id".to_string(),
        run_id.to_string(),
        "--chroot-base-dir".to_string(),
        workdir.to_string_lossy().to_string(),
        "--exec-file".to_string(),
        runtime.firecracker_bin.clone(),
    ];
    for op in &compile_bundle.jailer_plan.ops {
        jailer_args.push("--plan-op".to_string());
        jailer_args.push(op.clone());
    }
    jailer_args.push("--".to_string());
    jailer_args.extend(firecracker_args.clone());

    LaunchPlan {
        jailer: CommandSpec {
            program: runtime.jailer_bin.clone(),
            args: jailer_args,
        },
        firecracker: CommandSpec {
            program: runtime.firecracker_bin.clone(),
            args: firecracker_args,
        },
    }
}
