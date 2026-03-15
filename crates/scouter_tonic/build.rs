use std::{
    collections::hash_map::DefaultHasher,
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

    let hash_cache = PathBuf::from("src/generated/.proto_hash");
    let generated = PathBuf::from("src/generated/scouter.grpc.v1.rs");
    let descriptor_path = PathBuf::from("src/generated/scouter_descriptor.bin");
    let current_hash = hash_protos(protos).to_string();

    if generated.exists() && descriptor_path.exists() {
        let cached = std::fs::read_to_string(&hash_cache).unwrap_or_default();
        if cached.trim() == current_hash {
            return Ok(());
        }
    }

    // Always generate both server and client code. Feature-gating is handled
    // in lib.rs via #[cfg(feature = "server")] / #[cfg(feature = "client")].
    // This keeps the generated file identical regardless of which feature set
    // triggers the build script, preventing spurious regeneration when
    // scouter-tonic is compiled as a transitive dependency.
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .out_dir("src/generated")
        .compile_protos(protos, std::slice::from_ref(&proto_root))?;

    std::fs::write(&hash_cache, &current_hash)?;

    Ok(())
}
