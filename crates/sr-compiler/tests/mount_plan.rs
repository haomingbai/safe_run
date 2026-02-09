use sr_compiler::compile_dry_run;
use sr_policy::{
    Audit, Cpu, Memory, Metadata, Mount, Network, NetworkMode, PolicySpec, Resources, Runtime,
};

#[test]
fn compile_includes_mount_plan_in_order() {
    let policy = PolicySpec {
        api_version: "policy.safe-run.dev/v1alpha1".to_string(),
        metadata: Metadata {
            name: "demo".to_string(),
        },
        runtime: Runtime {
            command: "/bin/echo".to_string(),
            args: vec!["ok".to_string()],
        },
        resources: Resources {
            cpu: Cpu {
                max: "100000 100000".to_string(),
            },
            memory: Memory {
                max: "256Mi".to_string(),
            },
        },
        network: Network {
            mode: NetworkMode::None,
        },
        mounts: vec![
            Mount {
                source: "/var/lib/safe-run/input".to_string(),
                target: "/data/input".to_string(),
                read_only: true,
            },
            Mount {
                source: "/var/lib/safe-run/output".to_string(),
                target: "/data/output".to_string(),
                read_only: true,
            },
        ],
        audit: Audit {
            level: "basic".to_string(),
        },
    };

    let bundle = compile_dry_run(&policy).expect("compile bundle");
    assert!(bundle.mount_plan.enabled);
    assert_eq!(bundle.mount_plan.mounts.len(), 2);
    assert_eq!(
        bundle.mount_plan.mounts[0].source,
        "/var/lib/safe-run/input"
    );
    assert_eq!(bundle.mount_plan.mounts[0].target, "/data/input");
    assert!(bundle.mount_plan.mounts[0].read_only);
    assert_eq!(
        bundle.mount_plan.mounts[1].source,
        "/var/lib/safe-run/output"
    );
    assert_eq!(bundle.mount_plan.mounts[1].target, "/data/output");
    assert!(bundle.mount_plan.mounts[1].read_only);
}
