use std::{
    collections::hash_map::DefaultHasher,
    env,
    hash::{Hash, Hasher},
    path::PathBuf,
};

fn hash_protos(protos: &[PathBuf]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for path in protos {
        if let Ok(content) = std::fs::read(path) {
            content.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = PathBuf::from("proto");
    let protos = &[proto_root.join("grpc.v1.proto")];

    // Tell cargo to re-run this script only when proto files change.
    for proto in protos {
        println!("cargo:rerun-if-changed={}", proto.display());
    }

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

    // Skip code generation if the generated file already exists and the proto
    // content hasn't changed since the last successful generation. The hash is
    // stored in OUT_DIR (cargo's build cache) so it persists across invocations
    // without touching the source tree.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let hash_cache = out_dir.join(".proto_hash");
    let generated = PathBuf::from("src/generated/scouter.grpc.v1.rs");
    let current_hash = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        hash_protos(protos).hash(&mut hasher);
        // Include feature flags so different feature sets don't share a cache entry
        cfg!(feature = "server").hash(&mut hasher);
        cfg!(feature = "client").hash(&mut hasher);
        hasher.finish().to_string()
    };

    if generated.exists() {
        let cached = std::fs::read_to_string(&hash_cache).unwrap_or_default();
        if cached.trim() == current_hash {
            return Ok(());
        }
    }

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

    std::fs::write(&hash_cache, &current_hash)?;

    Ok(())
}
