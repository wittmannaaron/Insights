use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

#[derive(Serialize, Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub value: f64,
    pub display: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct TopProcessesResult {
    pub kind: String,
    pub processes: Vec<ProcessInfo>,
    pub note: Option<String>,
}

pub fn top_processes(kind: &str, limit: usize) -> TopProcessesResult {
    match kind {
        "cpu" => top_cpu(limit),
        "ram" => top_ram(limit),
        "gpu" | "vram" => TopProcessesResult {
            kind: kind.to_string(),
            processes: vec![],
            note: Some(
                "Per-process GPU metrics require sudo (powermetrics) — disabled by design."
                    .to_string(),
            ),
        },
        _ => TopProcessesResult {
            kind: kind.to_string(),
            processes: vec![],
            note: Some(format!("Unknown kind: {}", kind)),
        },
    }
}

fn top_cpu(limit: usize) -> TopProcessesResult {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new().with_cpu()),
    );
    // Two refreshes are needed to compute CPU usage delta.
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cpu(),
    );
    std::thread::sleep(std::time::Duration::from_millis(250));
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cpu(),
    );

    let core_count = sys.cpus().len().max(1) as f32;
    let mut rows: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, p)| {
            let pct = p.cpu_usage() / core_count;
            ProcessInfo {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().to_string(),
                value: pct as f64,
                display: format!("{:.1}%", pct),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    rows.truncate(limit);
    TopProcessesResult {
        kind: "cpu".to_string(),
        processes: rows,
        note: None,
    }
}

fn top_ram(limit: usize) -> TopProcessesResult {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new().with_memory()),
    );
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_memory(),
    );

    let mut rows: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, p)| {
            let bytes = p.memory();
            ProcessInfo {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().to_string(),
                value: bytes as f64,
                display: fmt_bytes(bytes),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    rows.truncate(limit);
    TopProcessesResult {
        kind: "ram".to_string(),
        processes: rows,
        note: None,
    }
}

fn fmt_bytes(b: u64) -> String {
    let mb = b as f64 / 1024.0 / 1024.0;
    if mb >= 1024.0 {
        format!("{:.2} GB", mb / 1024.0)
    } else {
        format!("{:.0} MB", mb)
    }
}
