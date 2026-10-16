#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadEffect {
    Notify,
    ScrollToBottom,
    ScheduleItemUpdate,
}
