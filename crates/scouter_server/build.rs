fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .out_dir("src/api/grpc/generated") // Custom location
        .compile_protos(&["proto/message.v1.proto"], &["proto"])?;
    Ok(())
}
