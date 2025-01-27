pub mod info;
pub mod print;
pub mod pushing;
pub mod system;

use info::InfoPayload;
use print::PrintPayload;
use pushing::PushingPayload;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use system::SystemPayload;

/// Our top-level `Command` enum uses `#[serde(untagged)]`.
/// Each variant is a different JSON root key:
///    { "info": {...} }
///    { "print": {...} }
///    { "pushing": {...} }
///    { "system": {...} }
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Command {
    /// e.g. `{"info": { "sequence_id": "123", "command": "get_version" }}`
    Info { info: InfoPayload },
    /// e.g. `{"print": { "sequence_id": "42", "command": "pause" }}`
    Print { print: PrintPayload },
    /// e.g. `{"pushing": { "sequence_id": "0", "command": "pushall" }}`
    Pushing { pushing: PushingPayload },
    /// e.g. `{"system": { "sequence_id": "77", "command": "ledctrl", ... }}`
    System { system: SystemPayload },
}

impl Command {
    /// Extracts the sequence ID from the command.
    pub(crate) fn sequence_id(&self) -> &SmolStr {
        match self {
            Command::Info { info } => &info.sequence_id,
            Command::Print { print } => &print.sequence_id,
            Command::Pushing { pushing } => &pushing.sequence_id,
            Command::System { system } => &system.sequence_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use info::InfoCommand;
    use print::PrintCommand;
    use pushing::PushingCommand;
    use serde_json::json;
    use system::{AccessoryType, LedCtrl, LedMode, LedNode, SystemCommand};

    #[test]
    fn test_get_version() {
        let cmd = Command::Info {
            info: InfoPayload {
                sequence_id: "999".into(),
                command: InfoCommand::GetVersion,
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "info": {
                "sequence_id": "999",
                "command": "get_version"
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_pause() {
        let cmd = Command::Print {
            print: PrintPayload {
                sequence_id: "123".into(),
                command: PrintCommand::Pause,
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "print": {
                "sequence_id": "123",
                "command": "pause"
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_resume() {
        let cmd = Command::Print {
            print: PrintPayload {
                sequence_id: "10".into(),
                command: PrintCommand::Resume,
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "print": {
                "sequence_id": "10",
                "command": "resume"
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_stop() {
        let cmd = Command::Print {
            print: PrintPayload {
                sequence_id: "7777".into(),
                command: PrintCommand::Stop,
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "print": {
                "sequence_id": "7777",
                "command": "stop"
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_push_all() {
        let cmd = Command::Pushing {
            pushing: PushingPayload {
                sequence_id: "555".into(),
                command: PushingCommand::PushAll,
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "pushing": {
                "sequence_id": "555",
                "command": "pushall"
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_start_push() {
        let cmd = Command::Pushing {
            pushing: PushingPayload {
                sequence_id: "2020".into(),
                command: PushingCommand::Start,
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "pushing": {
                "sequence_id": "2020",
                "command": "start"
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_set_chamber_light_on() {
        // For "true", we expect "led_mode": "on"
        let cmd = {
            // We can pick `LedMode::On` or `LedMode::Off` depending on `on: bool`
            let mode = if true { LedMode::On } else { LedMode::Off };
            Command::System {
                system: SystemPayload {
                    sequence_id: "42".into(),
                    command: SystemCommand::LedCtrl(LedCtrl {
                        led_node: LedNode::ChamberLight,
                        led_mode: mode,
                        led_on_time: 500,
                        led_off_time: 500,
                        loop_times: 0,
                        interval_time: 0,
                    }),
                },
            }
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "system": {
                "sequence_id": "42",
                "command": "ledctrl",
                "led_node": "chamber_light",
                "led_mode": "on",
                "led_on_time": 500,
                "led_off_time": 500,
                "loop_times": 0,
                "interval_time": 0
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_set_speed_profile() {
        // Original snippet used <PROFILE>. We can do "fast" or "superfast"
        let cmd = Command::Print {
            print: PrintPayload {
                sequence_id: "8910".into(),
                command: PrintCommand::PrintSpeed {
                    param: "fast".into(),
                },
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "print": {
                "sequence_id": "8910",
                "command": "print_speed",
                "param": "fast"
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_send_gcode_template() {
        // Original snippet used <GCODE>. Let's pick "G28" for homing
        let cmd = Command::Print {
            print: PrintPayload {
                sequence_id: "1234".into(),
                command: PrintCommand::GcodeLine {
                    param: "G28".into(),
                },
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "print": {
                "sequence_id": "1234",
                "command": "gcode_line",
                "param": "G28"
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_get_accessories() {
        let cmd = Command::System {
            system: SystemPayload {
                sequence_id: "9999".into(),
                command: SystemCommand::GetAccessories {
                    accessory_type: AccessoryType::None,
                },
            },
        };
        let actual = serde_json::to_value(&cmd).unwrap();
        let expected = json!({
            "system": {
                "sequence_id": "9999",
                "command": "get_accessories",
                "accessory_type": "none"
            }
        });
        assert_eq!(actual, expected);
    }
}
