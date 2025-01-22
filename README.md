# Bambu Proxy

Bambu Proxy is a Rust-based application that connects to a camera and streams MJPEG frames over HTTP. It uses the Tokio runtime for asynchronous operations and Axum for the web framework.

## Getting Started

### Prerequisites

- Rust and Cargo installed
- Set the following environment variables:
  - `BAMBU_IP`: IP address of the printer
  - `BAMBU_ACCESS_CODE`: Access code for the printer
  - `BAMBU_PORT`: Port for the camera (default: 6000)

### Building the Project

To build the project, run:

```sh
cargo build
```

### Running the Project

To run the project, use:

```
cargo run
```

### Usage

The application will start an HTTP server that streams MJPEG frames from the camera. Access the stream at:

```
http://<your_server_address>/live
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
