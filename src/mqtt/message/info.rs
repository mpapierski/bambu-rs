//! Info message structure.
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Module {
    pub name: SmolStr,
    pub project_name: SmolStr,
    pub sw_ver: SmolStr,
    pub hw_ver: SmolStr,
    pub sn: SmolStr,
    pub flag: u8,
    pub loader_ver: Option<SmolStr>,
    pub ota_ver: Option<SmolStr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Info {
    pub command: SmolStr,
    pub sequence_id: SmolStr,
    pub module: Vec<Module>,
    pub result: SmolStr,
    pub reason: SmolStr,
}
