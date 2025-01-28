# Bambu library

Bambu is a library for Rust ecosystem that interacts with BambuLab 3D printers that are LAN mode enabled.

## Supported features

- Interact with MQTT server to send requests and receive responses.
- Access the camera feed.
- Access files stored on the SD card.

## Getting Started

### Prerequisites

- Rust and Cargo installed.

### Building the Project

To build the project, run:

```sh
cargo build
```

### Running examples

One of the examples is a simple mjpeg stream server. Run it with:

```sh
export BAMBU_ACCESS_CODE=12345678
export BAMBU_IP=192.168.1.135
export BAMBU_SERIAL_NUMBER=123456789ABCDE
cargo run --example mjpeg_stream
```

### Running the Project

To run the tests, use:

```
cargo test
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Author

- Michał Papierski <michal@papierski.net>
