use crate::constants::{CGROUP_CPU_STAT_FILE, CGROUP_MEMORY_CURRENT_FILE};
use crate::constants::{EVENT_RESOURCE_SAMPLED, EVENT_RUN_FAILED, EVENT_VM_EXITED, STAGE_MONITOR};
use crate::event::write_event;
use crate::model::{MonitorResult, PreparedRun, RunState};
use nix::errno::Errno;
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use serde_json::json;
use sr_common::{ErrorItem, SR_RUN_001, SR_RUN_003};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
struct ResourceSample {
    cpu_usage_usec: u64,
    memory_current_bytes: u64,
}

/// Monitor a running VM process until exit or timeout.
/// The function emits `resource.sampled` events during polling and always emits `vm.exited`.
pub(crate) fn monitor_run(prepared: &mut PreparedRun) -> Result<MonitorResult, ErrorItem> {
    ensure_running_state(prepared)?;
    let vm_pid = read_vm_pid(prepared)?;
    let timeout = std::time::Duration::from_secs(prepared.runtime_context.timeout_sec);
    let sample_interval = prepared.runtime_context.effective_sample_interval();
    let started = Instant::now();
    let mut sample_count = 0u64;

    loop {
        if let Some(exit_code) = try_wait_exit_code(vm_pid)? {
            return finish_with_exit(prepared, exit_code, false, sample_count);
        }

        if started.elapsed() >= timeout {
            return handle_timeout(prepared, vm_pid, sample_count);
        }

        let sample = read_resource_sample(prepared)?;
        write_resource_sample_event(prepared, sample)?;
        sample_count += 1;
        thread::sleep(sample_interval);
    }
}

fn ensure_running_state(prepared: &PreparedRun) -> Result<(), ErrorItem> {
    if prepared.state != RunState::Running {
        return Err(ErrorItem::new(
            SR_RUN_001,
            "state",
            "runner monitor requires running state",
        ));
    }
    Ok(())
}

fn read_vm_pid(prepared: &PreparedRun) -> Result<Pid, ErrorItem> {
    let raw = fs::read_to_string(prepared.vm_pid_path()).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "monitor.vmPid",
            format!("failed to read vm pid artifact: {err}"),
        )
    })?;
    let pid = raw.trim().parse::<i32>().map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "monitor.vmPid",
            format!("invalid vm pid artifact content: {err}"),
        )
    })?;
    Ok(Pid::from_raw(pid))
}

fn try_wait_exit_code(pid: Pid) -> Result<Option<i32>, ErrorItem> {
    let status = waitpid(pid, Some(WaitPidFlag::WNOHANG)).map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "monitor.wait",
            format!("failed to poll vm process state: {err}"),
        )
    })?;
    Ok(match status {
        WaitStatus::StillAlive => None,
        WaitStatus::Exited(_, code) => Some(code),
        WaitStatus::Signaled(_, signal, _) => Some(128 + signal as i32),
        _ => Some(1),
    })
}

fn handle_timeout(
    prepared: &mut PreparedRun,
    pid: Pid,
    sample_count: u64,
) -> Result<MonitorResult, ErrorItem> {
    if let Err(err) = kill(pid, Signal::SIGKILL) {
        if err != Errno::ESRCH {
            return Err(ErrorItem::new(
                SR_RUN_003,
                "monitor.timeout.kill",
                format!("failed to terminate timed out vm process: {err}"),
            ));
        }
    }
    let exit_code = match waitpid(pid, None) {
        Ok(WaitStatus::Exited(_, code)) => code,
        Ok(WaitStatus::Signaled(_, signal, _)) => 128 + signal as i32,
        Ok(_) => 137,
        Err(_) => 137,
    };
    let _ = finish_with_exit(prepared, exit_code, true, sample_count);
    let _ = write_event(
        prepared,
        STAGE_MONITOR,
        EVENT_RUN_FAILED,
        json!({
            "reason": "timeout",
            "errorCode": SR_RUN_003,
            "timeoutSec": prepared.runtime_context.timeout_sec
        }),
    );
    Err(ErrorItem::new(
        SR_RUN_003,
        "monitor.timeout",
        format!(
            "run timed out after {} seconds",
            prepared.runtime_context.timeout_sec
        ),
    ))
}

fn finish_with_exit(
    prepared: &mut PreparedRun,
    exit_code: i32,
    timed_out: bool,
    sample_count: u64,
) -> Result<MonitorResult, ErrorItem> {
    prepared.state = if exit_code == 0 && !timed_out {
        RunState::Finished
    } else {
        RunState::Failed
    };
    let result = MonitorResult {
        exit_code,
        timed_out,
        sample_count,
    };
    write_vm_exited_event(prepared, &result)?;
    if result.exit_code != 0 && !result.timed_out {
        write_event(
            prepared,
            STAGE_MONITOR,
            EVENT_RUN_FAILED,
            json!({
                "reason": "abnormal_exit",
                "errorCode": SR_RUN_001,
                "exitCode": result.exit_code
            }),
        )?;
    }
    Ok(result)
}

fn read_resource_sample(prepared: &PreparedRun) -> Result<ResourceSample, ErrorItem> {
    let cgroup_path = prepared.runtime_context.effective_cgroup_path();
    let cgroup_root = Path::new(&cgroup_path);
    let cpu_stat_raw =
        fs::read_to_string(cgroup_root.join(CGROUP_CPU_STAT_FILE)).map_err(|err| {
            ErrorItem::new(
                SR_RUN_001,
                "monitor.cgroup.cpu",
                format!("failed to read cgroup cpu.stat: {err}"),
            )
        })?;
    let memory_raw =
        fs::read_to_string(cgroup_root.join(CGROUP_MEMORY_CURRENT_FILE)).map_err(|err| {
            ErrorItem::new(
                SR_RUN_001,
                "monitor.cgroup.memory",
                format!("failed to read cgroup memory.current: {err}"),
            )
        })?;
    let cpu_usage_usec = parse_cpu_usage_usec(&cpu_stat_raw)?;
    let memory_current_bytes = parse_memory_current(&memory_raw)?;
    Ok(ResourceSample {
        cpu_usage_usec,
        memory_current_bytes,
    })
}

fn parse_cpu_usage_usec(raw: &str) -> Result<u64, ErrorItem> {
    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        if key == "usage_usec" {
            return value.parse::<u64>().map_err(|err| {
                ErrorItem::new(
                    SR_RUN_001,
                    "monitor.cgroup.cpu",
                    format!("invalid usage_usec value in cpu.stat: {err}"),
                )
            });
        }
    }
    Err(ErrorItem::new(
        SR_RUN_001,
        "monitor.cgroup.cpu",
        "cpu.stat is missing usage_usec field",
    ))
}

fn parse_memory_current(raw: &str) -> Result<u64, ErrorItem> {
    raw.trim().parse::<u64>().map_err(|err| {
        ErrorItem::new(
            SR_RUN_001,
            "monitor.cgroup.memory",
            format!("invalid memory.current value: {err}"),
        )
    })
}

fn write_resource_sample_event(
    prepared: &mut PreparedRun,
    sample: ResourceSample,
) -> Result<(), ErrorItem> {
    write_event(
        prepared,
        STAGE_MONITOR,
        EVENT_RESOURCE_SAMPLED,
        json!({
            "cpuUsageUsec": sample.cpu_usage_usec,
            "memoryCurrentBytes": sample.memory_current_bytes,
            "cgroupPath": prepared.runtime_context.effective_cgroup_path()
        }),
    )
}

fn write_vm_exited_event(
    prepared: &mut PreparedRun,
    result: &MonitorResult,
) -> Result<(), ErrorItem> {
    write_event(
        prepared,
        STAGE_MONITOR,
        EVENT_VM_EXITED,
        json!({
            "exitCode": result.exit_code,
            "timedOut": result.timed_out,
            "sampleCount": result.sample_count
        }),
    )
}
