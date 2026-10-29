//! Read-only, best-effort measurements. System/device use includes other applications.
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use tokio::io::AsyncReadExt;

pub struct Sampler {
    previous_cpu: Option<(u64, u64)>,
    gpu: Option<PathBuf>,
}
impl Sampler {
    pub fn new() -> Self {
        // Use driver installation locations, never a workspace executable or arbitrary PATH entry.
        let gpu = [
            std::env::var_os("SystemRoot")
                .map(|v| PathBuf::from(v).join("System32/nvidia-smi.exe")),
            std::env::var_os("ProgramW6432")
                .map(|v| PathBuf::from(v).join("NVIDIA Corporation/NVSMI/nvidia-smi.exe")),
        ]
        .into_iter()
        .flatten()
        .find(|p| p.is_file());
        Self {
            previous_cpu: None,
            gpu,
        }
    }
    pub fn system(&mut self) -> Value {
        let (cpu, memory, app_bytes) = system();
        let percent =
            cpu.zip(self.previous_cpu)
                .and_then(|((idle, total), (old_idle, old_total))| {
                    let total = total.checked_sub(old_total)?;
                    let idle = idle.checked_sub(old_idle)?;
                    (total > 0 && idle <= total)
                        .then(|| (total - idle) as f64 * 100. / total as f64)
                });
        self.previous_cpu = cpu;
        json!({"systemCpuPercent":percent,"systemMemory":memory,"appWorkingSetBytes":app_bytes,
            "cpuScope":"all_logical_processors_current_group","cpuReason":if percent.is_none(){Some("baseline_or_counter_unavailable")}else{None},
            "gpuProbe":false,"gpus":null,"gpuReason":"not_polled_at_boundary"})
    }
    pub async fn gpu(&self) -> Value {
        let Some(path) = &self.gpu else {
            return json!({"reason":"nvidia_smi_not_installed","devices":null});
        };
        let result = async {
            let mut command = tokio::process::Command::new(path);
            command
                .args([
                    "--query-gpu=uuid,memory.used,memory.total,utilization.gpu,driver_version",
                    "--format=csv,noheader,nounits",
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true);
            #[cfg(windows)]
            command.creation_flags(0x08000000);
            let mut child = command.spawn().map_err(|_| "nvidia_smi_start_failed")?;
            let mut bytes = vec![];
            child
                .stdout
                .take()
                .ok_or("nvidia_smi_no_output")?
                .take(65537)
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| "nvidia_smi_read_failed")?;
            if bytes.len() > 65536 {
                return Err("nvidia_smi_output_limit");
            }
            if !child
                .wait()
                .await
                .map_err(|_| "nvidia_smi_wait_failed")?
                .success()
            {
                return Err("nvidia_smi_failed");
            }
            parse_gpu(std::str::from_utf8(&bytes).map_err(|_| "nvidia_smi_invalid_utf8")?)
        };
        match tokio::time::timeout(Duration::from_millis(750), result).await {
            Ok(Ok(devices)) => json!({"devices":devices,"reason":null}),
            Ok(Err(reason)) => json!({"devices":null,"reason":reason}),
            Err(_) => json!({"devices":null,"reason":"nvidia_smi_timeout"}),
        }
    }
}
fn parse_gpu(text: &str) -> Result<Vec<Value>, &'static str> {
    let mib = |v: &str| {
        v.parse::<u64>()
            .ok()
            .and_then(|v| v.checked_mul(1024 * 1024))
    };
    let mut devices = vec![];
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let parts: Vec<_> = line.split(',').map(str::trim).collect();
        if parts.len() != 5 || !parts[0].starts_with("GPU-") || devices.len() >= 64 {
            return Err("nvidia_smi_invalid_output");
        }
        devices.push(json!({"uuid":parts[0],"usedBytes":mib(parts[1]),"totalBytes":mib(parts[2]),
            "utilizationPercent":parts[3].parse::<f64>().ok().filter(|v|v.is_finite()&&(0. ..=100.).contains(v)),"driverVersion":parts[4]}));
    }
    if devices.is_empty() {
        Err("nvidia_smi_no_devices")
    } else {
        Ok(devices)
    }
}

#[cfg(windows)]
fn system() -> (Option<(u64, u64)>, Value, Option<u64>) {
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
            Threading::{GetCurrentProcess, GetSystemTimes},
        },
    };
    // The structs are POD, size fields are required by the Windows APIs.
    unsafe {
        let (mut idle, mut kernel, mut user): (FILETIME, FILETIME, FILETIME) =
            (std::mem::zeroed(), std::mem::zeroed(), std::mem::zeroed());
        let ticks = |v: FILETIME| ((v.dwHighDateTime as u64) << 32) | v.dwLowDateTime as u64;
        let cpu = (GetSystemTimes(&mut idle, &mut kernel, &mut user) != 0)
            .then(|| (ticks(idle), ticks(kernel) + ticks(user)));
        let mut memory: MEMORYSTATUSEX = std::mem::zeroed();
        memory.dwLength = std::mem::size_of_val(&memory) as u32;
        let memory = if GlobalMemoryStatusEx(&mut memory) != 0 {
            json!({"usedBytes":memory.ullTotalPhys-memory.ullAvailPhys,"totalBytes":memory.ullTotalPhys})
        } else {
            Value::Null
        };
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of_val(&counters) as u32;
        let app = (K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0)
            .then_some(counters.WorkingSetSize as u64);
        (cpu, memory, app)
    }
}
#[cfg(not(windows))]
fn system() -> (Option<(u64, u64)>, Value, Option<u64>) {
    (None, Value::Null, None)
}

pub fn hardware_snapshot() -> Value {
    let (_, memory, _) = system();
    json!({"schemaVersion":1,"source":"windows_api","os":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "cpu":std::env::var("PROCESSOR_IDENTIFIER").ok(),"logicalProcessors":std::thread::available_parallelism().ok().map(|n|n.get()),
        "totalMemoryBytes":memory["totalBytes"],"gpuIdentity":"see_timestamped_resource_samples","sampleIntervalMs":1000,
        "resourceScope":"system_and_whole_gpu_not_model_exclusive","appScope":"tauri_process_only_excludes_webview_and_ollama"})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn telemetry_gpu_parser_preserves_unsupported_fields() {
        let d = parse_gpu("GPU-test, 12, 8192, [N/A], 123.4\n").unwrap();
        assert_eq!(d[0]["usedBytes"], 12 * 1024 * 1024);
        assert!(d[0]["utilizationPercent"].is_null());
        assert!(parse_gpu("garbage").is_err());
        assert!(parse_gpu("").is_err());
    }
}
