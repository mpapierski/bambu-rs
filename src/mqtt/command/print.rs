use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Serialize, Deserialize)]
pub struct PrintPayload {
    pub sequence_id: SmolStr,

    #[serde(flatten)]
    pub command: PrintCommand,
}

/// Possible commands for the "print" JSON root.
/// Some have an extra `param` field.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum PrintCommand {
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "resume")]
    Resume,
    #[serde(rename = "stop")]
    Stop,

    // "print_speed" -> has param
    #[serde(rename = "print_speed")]
    PrintSpeed { param: SmolStr },
    // "gcode_line" -> has param
    #[serde(rename = "gcode_line")]
    GcodeLine { param: SmolStr },
    // "extrusion_cali_get" -> has param
    #[serde(rename = "extrusion_cali_get")]
    ExtrusionCalibrationGet {
        filament_id: SmolStr,
        nozzle_diameter: SmolStr,
    },
}
