use bambu::MqttClient;

pub(crate) struct Config {
    pub(crate) printer_ip: String,
    pub(crate) access_code: String,
    #[allow(dead_code)]
    pub(crate) serial_number: String,
    #[allow(dead_code)]
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

#[tokio::main]
async fn main() {
    let config = Config::from_env();

    let mut client = MqttClient::new(
        &config.printer_ip,
        &config.access_code,
        &config.serial_number,
    );
    let task = client.start().await.expect("Start failed");

    client.push_all().await.expect("foo");

    println!("Client started");

    // let request = client.get_version().await.expect("Get version failed");
    // println!("Get version: {:?}", request);

    // let message = client
    //     .extrusion_calibration_get("", "0")
    //     .await
    //     .expect("Push all failed");
    // println!("Push all: {:?}", message);

    task.await.expect("Task failed");
}
