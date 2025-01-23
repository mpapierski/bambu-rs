pub(crate) struct Config {
    pub(crate) printer_ip: String,
    pub(crate) access_code: String,
    pub(crate) serial_number: String,
    pub(crate) camera_port: u16,
}

impl Config {
    pub(crate) fn from_env() -> Self {
        let printer_ip = std::env::var("BAMBU_IP").expect("BAMBU_IP must be set");
        let access_code =
            std::env::var("BAMBU_ACCESS_CODE").expect("BAMBU_ACCESS_CODE must be set");
        let serial_number =
            std::env::var("BAMBU_SERIAL_NUMBER").expect("BAMBU_SERIAL_NUMBER must be set");
        let camera_port: u16 = std::env::var("BAMBU_PORT")
            .unwrap_or_else(|_| "6000".to_string())
            .parse()
            .expect("BAMBU_PORT must be a valid u16");

        Self {
            printer_ip,
            access_code,
            serial_number,
            camera_port,
        }
    }
}
