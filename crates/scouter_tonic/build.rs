use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = PathBuf::from("proto");
    let protos = &[proto_root.join("grpc.v1.proto")];

    let missing: Vec<_> = protos.iter().filter(|p| !p.exists()).collect();
    if !missing.is_empty() {
        panic!(
            "Proto files not found:\n{}",
            missing
                .iter()
                .map(|p| format!("  - {p:?}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let descriptor_path = out_dir.join("scouter_descriptor.bin");

    let mut config = tonic_prost_build::configure();

    #[cfg(feature = "server")]
    {
        config = config.build_server(true);
    }
    #[cfg(not(feature = "server"))]
    {
        config = config.build_server(false);
    }

    #[cfg(feature = "client")]
    {
        config = config.build_client(true);
    }
    #[cfg(not(feature = "client"))]
    {
        config = config.build_client(false);
    }

    config
        .file_descriptor_set_path(&descriptor_path)
        .out_dir("src/generated")
        .compile_protos(protos, std::slice::from_ref(&proto_root))?;

    println!("cargo:rerun-if-changed={}", proto_root.display());

    Ok(())
}
