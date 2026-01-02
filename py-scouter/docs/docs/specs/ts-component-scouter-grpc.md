# Technical Specification: gRPC/Tonic Integration

## Overview
This specification documents the integration of gRPC/Tonic into the Scouter ecosystem, providing a high-performance transport layer for real-time data transport. The scouter-tonic crate serves as the foundation for gRPC communication between Scouter clients (Python/Rust) and the Scouter server, offering compile-time type safety, efficient binary serialization via Protocol Buffers, and seamless interoperability across language boundaries.

### Rationale
Scouter is build on the idea "quality control for AI monitoring and observability". Within that, a key component is building robust system that are reliable and performant. Along those lines, we aim to provide highly-performant workflows for recording data in client applications. We currently support, HTTP, Kafka, RabbitMQ, and Redis as transport layers for sending data to the Scouter server. gRPC is a natural fit for this ecosystem and provides a performant middle ground between HTTP and message brokers. Note - gRPC and HTTP are now standard implementations with Scouter, and Kafka/RabbitMQ/Redis require addition setup and feature flags to enable.


## Key Additions

- New scouter-tonic Crate: Standalone crate containing gRPC service definitions and generated client/server code
- Protocol Buffer Definitions: Centralized .proto files defining service contracts (proto/grpc.v1.proto)
- Feature-Gated Compilation: Conditional compilation of client/server code via cargo features (client, server, all)
- Python Integration: PyO3 bindings exposing GrpcConfig and GrpcSpanExporter for Python consumers
- Transport Abstraction: Unified transport layer supporting HTTP, gRPC, Kafka, RabbitMQ, and Redis
- Production-Ready Defaults: Authentication support, error handling, and configuration management

## Architecture

```
scouter_tonic/
├── proto/
│   └── grpc.v1.proto          # Service & message definitions
├── src/
│   ├── generated/             # Auto-generated Rust code
│   │   ├── scouter.grpc.v1.rs
│   │   └── scouter.message.v1.rs
│   ├── client.rs              # gRPC client implementation
│   ├── error.rs               # Error types & conversions
│   └── lib.rs                 # Public API exports
├── build.rs                   # Protobuf compilation script
└── Cargo.toml                 # Dependencies & features
```

## Dependencies
The scouter-tonic crate currently relies on the tonic ecosystem for gRPC functionality, as it is the current standard for Rust gRPC implementations. As the Rust gRPC ecosystem evolves with google starting to build out a Rust gRPC implementation, we will evaluate alternatives to tonic for potential migration in the future.


## Implementation Details

### 1. Build Script (build.rs)
The build script orchestrates Protocol Buffer compilation with feature-gated code generation:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = tonic_prost_build::configure();

    // Only generate server code if the "server" feature is enabled
    #[cfg(feature = "server")]
    {
        config = config.build_server(true);
    }
    #[cfg(not(feature = "server"))]
    {
        config = config.build_server(false);
    }

    // Only generate client code if the "client" feature is enabled
    #[cfg(feature = "client")]
    {
        config = config.build_client(true);
    }
    #[cfg(not(feature = "client"))]
    {
        config = config.build_client(false);
    }

    config
        .out_dir("src/generated")
        .compile_protos(&["proto/grpc.v1.proto"], &["proto"])?;

    Ok(())
}
```

#### Benefits:

- Reduced Binary Size: Only compile client or server code as needed
- Faster Build Times: Avoid unnecessary codegen for unused features
- Clean Separation: Client-only libraries don't include server dependencies

### 2. Client Module (client.rs)
The client module encapsulates gRPC client functionality, providing a high-level API for sending data to the Scouter server. Importantly, it also includes authentication support via metadata headers and a login mechanism given the server requires authentication.

### 3. Server Module (scouter_server/grpc/mod.rs)
The gRPC server is implemented within the scouter_server crate, and runs alongside the existing HTTP server (Axum) with a default port of 50051. It listens for incoming gRPC requests, authenticates clients with an Interceptor middleware, and processes data submissions.

### 4. Python Integration
Grpc is typically used when (1) exporting spans to Scouter server for tracing, or (2) sending monitoring data (features, metrics, genai records) to the Scouter server. To facilitate this, we have exposed one main classes to the Python layer via PyO3:

**GrpcConfig**: Configuration class for setting up gRPC transport to the Scouter server. This class allows users to specify server address, username and password. These are also pulled from the environment variables `SCOUTER_GRPC_URI`, `SCOUTER_USERNAME`, and `SCOUTER_PASSWORD` if not provided directly.

```python
class GrpcConfig:
    server_uri: str
    username: str
    password: str

    def __init__(
        self,
        server_uri: Optional[str] = None,
        username: Optional[str] = None,
        password: Optional[str] = None,
    ) -> None:
        """gRPC configuration to use with the GrpcProducer.

        Args:
            server_uri:
                URL of the gRPC server to publish messages to.
                If not provided, the value of the SCOUTER_GRPC_URI environment variable is used.

            username:
                Username for basic authentication.
                If not provided, the value of the SCOUTER_USERNAME environment variable is used.

            password:
                Password for basic authentication.
                If not provided, the value of the SCOUTER_PASSWORD environment variable is used.
        """
```

When passed as a transport configuration to a `ScouterQueue` or `init_tracer`, the `GrpcConfig` will be used to initialize a gRPC client and producer that sends monitoring data to the Scouter server over gRPC.

```python
from scouter import ScouterQueue, Features, Feature
from scouter.transport import GrpcConfig

# Configure gRPC transport
config = GrpcConfig(
    server_uri="http://localhost:9090"
)

# Create queue with gRPC transport
queue = ScouterQueue.from_path(
    {"model-v1": Path("profile.json")},
    transport_config=config
)

# Insert features (async publish via gRPC)
features = Features(
    features=[
        Feature.float("temperature", 23.5),
        Feature.int("requests", 1200),
        Feature.string("region", "us-west-2"),
    ]
)
queue["model-v1"].insert(features)

# Graceful shutdown
queue.shutdown()
```

---

*Version: 1.0*
*Last Updated: 2025-12-18*
*Component Owner: Steven Forrester*