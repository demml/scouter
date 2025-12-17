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
        .compile_protos(&["proto/message.v1.proto"], &["proto"])?;

    Ok(())
}
