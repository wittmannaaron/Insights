use serde::Serialize;
use std::time::Duration;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tauri::{async_runtime, AppHandle, Emitter, Runtime};

use crate::state::AppState;

#[derive(Serialize, Clone)]
pub struct CpuSample {
    pub usage_pct: f32,
    pub ts_ms: u128,
}

#[derive(Serialize, Clone)]
pub struct RamSample {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub used_pct: f32,
    pub ts_ms: u128,
}

pub fn spawn<R: Runtime>(app: AppHandle<R>, state: AppState) {
    async_runtime::spawn(async move {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::new().with_ram()),
        );
        // sysinfo CPU usage requires two refreshes with at least MINIMUM_CPU_UPDATE_INTERVAL between.
        sys.refresh_cpu_usage();
        tokio::time::sleep(Duration::from_millis(250)).await;

        loop {
            sys.refresh_cpu_usage();
            sys.refresh_memory();

            let cpu_usage =
                sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;

            let used = sys.used_memory();
            let total = sys.total_memory().max(1);
            let used_pct = (used as f64 / total as f64 * 100.0) as f32;

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);

            let _ = app.emit(
                "metrics:cpu",
                CpuSample {
                    usage_pct: cpu_usage,
                    ts_ms: now_ms,
                },
            );
            let _ = app.emit(
                "metrics:ram",
                RamSample {
                    used_bytes: used,
                    total_bytes: total,
                    used_pct,
                    ts_ms: now_ms,
                },
            );

            tokio::time::sleep(state.sample_interval()).await;
        }
    });
}
