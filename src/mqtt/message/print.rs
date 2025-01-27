//! Print message.
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Represents the printer status.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Print {
    pub bed_temper: Option<f64>,
    pub nozzle_temper: Option<f64>,
    pub command: SmolStr,
    pub msg: u64,
    pub sequence_id: SmolStr,
}
