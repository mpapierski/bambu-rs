use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemPayload {
    pub sequence_id: SmolStr,

    #[serde(flatten)]
    pub command: SystemCommand,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command")]
/// "ledctrl" for turning on/off the chamber light, etc.
pub struct LedCtrl {
    pub led_node: LedNode,
    pub led_mode: LedMode,
    pub led_on_time: u32,
    pub led_off_time: u32,
    pub loop_times: u32,
    pub interval_time: u32,
}

/// Enum for system‐level commands like `ledctrl` and `get_accessories`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum SystemCommand {
    #[serde(rename = "ledctrl")]
    LedCtrl(LedCtrl),
    /// "get_accessories"
    #[serde(rename = "get_accessories")]
    GetAccessories { accessory_type: AccessoryType },
}

/// Instead of `led_node` being a &str, we define a small enum for valid LED nodes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedNode {
    ChamberLight,
}

/// Instead of `led_mode` being a String, we define another enum for valid modes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedMode {
    On,
    Off,
}

/// Same approach for "accessory_type" instead of a static str:
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessoryType {
    None,
}
