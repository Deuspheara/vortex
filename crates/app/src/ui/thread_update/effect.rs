#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadEffect {
    Notify,
    ScrollToBottom,
    ScheduleStreamSync { interval_ms: u64 },
    ScheduleItemUpdate,
}
