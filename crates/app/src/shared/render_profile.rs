use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(debug_assertions)]
const REPORT_INTERVAL: Duration = Duration::from_secs(1);

#[cfg(debug_assertions)]
#[derive(Clone, Copy, Default)]
struct RenderMetric {
    count: u64,
    total_us: u128,
    max_us: u128,
    rows: u64,
}

#[cfg(debug_assertions)]
#[derive(Default)]
struct RenderProfileState {
    enabled: Option<bool>,
    last_report: Option<Instant>,
    metrics: HashMap<&'static str, RenderMetric>,
}

#[cfg(debug_assertions)]
static STATE: OnceLock<Mutex<RenderProfileState>> = OnceLock::new();

#[cfg(debug_assertions)]
fn state() -> &'static Mutex<RenderProfileState> {
    STATE.get_or_init(|| Mutex::new(RenderProfileState::default()))
}

#[cfg(debug_assertions)]
fn enabled_inner(state: &mut RenderProfileState) -> bool {
    *state.enabled.get_or_insert_with(|| {
        std::env::var("VORTEX_RENDER_PROFILE")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

#[cfg(debug_assertions)]
pub fn enabled() -> bool {
    let Ok(mut guard) = state().lock() else {
        return false;
    };
    enabled_inner(&mut guard)
}

#[cfg(not(debug_assertions))]
pub fn enabled() -> bool {
    false
}

pub struct RenderProfileSpan {
    #[cfg(debug_assertions)]
    label: &'static str,
    #[cfg(debug_assertions)]
    start: Option<Instant>,
}

impl RenderProfileSpan {
    pub fn new(label: &'static str) -> Self {
        #[cfg(debug_assertions)]
        {
            return Self {
                label,
                start: enabled().then(Instant::now),
            };
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = label;
            Self {}
        }
    }
}

impl Drop for RenderProfileSpan {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            let Some(start) = self.start else {
                return;
            };
            record(self.label, start.elapsed(), 0);
        }
    }
}

pub fn span(label: &'static str) -> RenderProfileSpan {
    RenderProfileSpan::new(label)
}

#[cfg(debug_assertions)]
pub fn record(label: &'static str, elapsed: Duration, rows: u64) {
    let Ok(mut guard) = state().lock() else {
        return;
    };
    if !enabled_inner(&mut guard) {
        return;
    }

    let metric = guard.metrics.entry(label).or_default();
    let elapsed_us = elapsed.as_micros();
    metric.count += 1;
    metric.total_us += elapsed_us;
    metric.max_us = metric.max_us.max(elapsed_us);
    metric.rows += rows;

    let now = Instant::now();
    let should_report = guard
        .last_report
        .map(|last| now.duration_since(last) >= REPORT_INTERVAL)
        .unwrap_or(true);
    if !should_report {
        return;
    }
    guard.last_report = Some(now);
    let mut lines = guard
        .metrics
        .iter()
        .map(|(name, metric)| {
            let avg_us = metric.total_us / u128::from(metric.count.max(1));
            let avg_rows = metric.rows / metric.count.max(1);
            format!(
                "{name}: count={} avg={:.2}ms max={:.2}ms rows_avg={avg_rows}",
                metric.count,
                avg_us as f64 / 1000.0,
                metric.max_us as f64 / 1000.0
            )
        })
        .collect::<Vec<_>>();
    lines.sort();
    tracing::debug!("render profile\n{}", lines.join("\n"));
    guard.metrics.clear();
}

#[cfg(not(debug_assertions))]
pub fn record(_label: &'static str, _elapsed: Duration, _rows: u64) {}
