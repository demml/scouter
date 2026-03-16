use std::path::PathBuf;

fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    data.iter()
        .fold(FNV_OFFSET, |acc, &b| (acc ^ b as u64).wrapping_mul(FNV_PRIME))
}

fn hash_protos(protos: &[PathBuf]) -> Result<u64, Box<dyn std::error::Error>> {
    let mut hash: u64 = 0;
    for path in protos {
        let content = std::fs::read(path)?;
        hash ^= fnv1a_hash(&content);
    }
    Ok(hash)
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
    let current_hash = hash_protos(protos)?.to_string();

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
