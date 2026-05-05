use serde::Serialize;
use std::process::Command;
use std::time::Duration;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};
use tauri::{async_runtime, AppHandle, Emitter, Runtime};

#[derive(Serialize, Clone)]
pub struct GpuSample {
    pub usage_pct: f32,
    pub ts_ms: u128,
}

#[derive(Serialize, Clone)]
pub struct VramSample {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub used_pct: f32,
    pub ts_ms: u128,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct PerfStats {
    pub device_util_pct: f32,
    pub in_use_bytes: u64,
}

pub fn parse_perf_stats(ioreg_output: &str) -> PerfStats {
    let mut out = PerfStats::default();
    if let Some(util) = extract_kv_int(ioreg_output, "Device Utilization %") {
        out.device_util_pct = util as f32;
    }
    if let Some(used) = extract_kv_int(ioreg_output, "In use system memory") {
        out.in_use_bytes = used as u64;
    }
    out
}

fn extract_kv_int(haystack: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{}\"=", key);
    let idx = haystack.find(&needle)?;
    let rest = &haystack[idx + needle.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse::<i64>().ok()
}

fn run_ioreg() -> Option<String> {
    let out = Command::new("ioreg")
        .args(["-rw0", "-c", "IOAccelerator"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    async_runtime::spawn(async move {
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_memory(MemoryRefreshKind::new().with_ram()),
        );
        loop {
            sys.refresh_memory();
            let total_ram = sys.total_memory().max(1);

            let stats = run_ioreg()
                .as_deref()
                .map(parse_perf_stats)
                .unwrap_or_default();

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);

            let _ = app.emit(
                "metrics:gpu",
                GpuSample {
                    usage_pct: stats.device_util_pct,
                    ts_ms: now_ms,
                },
            );

            let used_pct = (stats.in_use_bytes as f64 / total_ram as f64 * 100.0) as f32;
            let _ = app.emit(
                "metrics:vram",
                VramSample {
                    used_bytes: stats.in_use_bytes,
                    total_bytes: total_ram,
                    used_pct,
                    ts_ms: now_ms,
                },
            );

            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
        | "PerformanceStatistics" = {"In use system memory (driver)"=0,"Alloc system memory"=41632137216,"Tiler Utilization %"=2,"recoveryCount"=0,"lastRecoveryTime"=0,"Renderer Utilization %"=3,"TiledSceneBytes"=1703936,"Device Utilization %"=42,"SplitSceneCount"=0,"Allocated PB Size"=243138560,"In use system memory"=1116782592}
    "#;

    #[test]
    fn parses_device_util() {
        let s = parse_perf_stats(FIXTURE);
        assert_eq!(s.device_util_pct, 42.0);
    }

    #[test]
    fn parses_in_use_bytes() {
        let s = parse_perf_stats(FIXTURE);
        assert_eq!(s.in_use_bytes, 1116782592);
    }

    #[test]
    fn empty_input_returns_zero() {
        let s = parse_perf_stats("");
        assert_eq!(s.device_util_pct, 0.0);
        assert_eq!(s.in_use_bytes, 0);
    }
}
