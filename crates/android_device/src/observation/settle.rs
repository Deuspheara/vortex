use std::future::Future;
use std::time::{Duration, Instant};

use agent_protocol::UiSettleResult;
use tokio::time::sleep;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettleSnapshot {
    pub screenshot_hash: String,
    pub tree_hash: String,
    pub package: Option<String>,
    pub activity: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UiSettleConfig {
    pub min_wait: Duration,
    pub screenshot_poll: Duration,
    pub stable_for: Duration,
    pub hard_timeout: Duration,
}

impl Default for UiSettleConfig {
    fn default() -> Self {
        Self {
            min_wait: Duration::from_millis(150),
            screenshot_poll: Duration::from_millis(100),
            stable_for: Duration::from_millis(400),
            hard_timeout: Duration::from_secs(5),
        }
    }
}

pub async fn wait_for_ui_settle<F, Fut>(
    before: SettleSnapshot,
    config: UiSettleConfig,
    mut observe: F,
) -> Result<UiSettleResult, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<SettleSnapshot, String>>,
{
    sleep(config.min_wait).await;
    let start = Instant::now();
    let mut last = observe().await?;
    let mut stable_since = Instant::now();

    loop {
        if start.elapsed() >= config.hard_timeout {
            return Ok(result(false, start.elapsed(), &before, &last));
        }

        sleep(config.screenshot_poll).await;
        let next = observe().await?;
        if next == last {
            if stable_since.elapsed() >= config.stable_for {
                return Ok(result(true, start.elapsed(), &before, &next));
            }
        } else {
            stable_since = Instant::now();
            last = next;
        }
    }
}

fn result(
    stable: bool,
    duration: Duration,
    before: &SettleSnapshot,
    after: &SettleSnapshot,
) -> UiSettleResult {
    UiSettleResult {
        stable,
        duration_ms: duration.as_millis() as u64,
        screenshot_changed: before.screenshot_hash != after.screenshot_hash,
        tree_changed: before.tree_hash != after.tree_hash,
        package_changed: before.package != after.package,
        activity_changed: before.activity != after.activity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reports_stable_after_matching_snapshots() {
        let before = SettleSnapshot {
            screenshot_hash: "a".into(),
            tree_hash: "a".into(),
            package: Some("pkg".into()),
            activity: Some("A".into()),
        };
        let after = SettleSnapshot {
            screenshot_hash: "b".into(),
            tree_hash: "a".into(),
            package: Some("pkg".into()),
            activity: Some("B".into()),
        };
        let config = UiSettleConfig {
            min_wait: Duration::from_millis(1),
            screenshot_poll: Duration::from_millis(1),
            stable_for: Duration::from_millis(2),
            hard_timeout: Duration::from_millis(50),
        };
        let result = wait_for_ui_settle(before, config, || {
            let after = after.clone();
            async move { Ok(after) }
        })
        .await
        .expect("settle");
        assert!(result.stable);
        assert!(result.screenshot_changed);
        assert!(result.activity_changed);
    }
}
