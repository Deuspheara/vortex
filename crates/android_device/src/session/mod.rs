use agent_protocol::AndroidDeviceRef;

#[derive(Clone, Debug, Default)]
pub struct DeviceRegistry {
    pub selected: Option<AndroidDeviceRef>,
    pub devices: Vec<AndroidDeviceRef>,
}
