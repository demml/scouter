use std::path::PathBuf;

fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    data.iter()
        .fold(FNV_OFFSET, |acc, &b| acc.wrapping_mul(FNV_PRIME) ^ b as u64)
}

fn hash_protos(protos: &[PathBuf]) -> u64 {
    let mut hash: u64 = 0;
    for path in protos {
        if let Ok(content) = std::fs::read(path) {
            hash ^= fnv1a_hash(&content);
        }
    }
    hash
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
            // Generated file is up-to-date. Still need scouter_descriptor.bin
            // in OUT_DIR for the include_bytes! in lib.rs — copy it from the
            // last generation if present alongside the hash cache.
            let cached_descriptor = hash_cache.with_file_name("scouter_descriptor.bin");
            let descriptor_path = out_dir.join("scouter_descriptor.bin");
            if cached_descriptor.exists() && !descriptor_path.exists() {
                std::fs::copy(&cached_descriptor, &descriptor_path)?;
            }
            if descriptor_path.exists() {
                return Ok(());
            }
            // Descriptor missing even after copy attempt — fall through to regenerate.
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

    // Cache the descriptor next to the hash so future compilations with a
    // different OUT_DIR (different profile / feature set) can reuse it.
    let cached_descriptor = hash_cache.with_file_name("scouter_descriptor.bin");
    std::fs::copy(&descriptor_path, &cached_descriptor)?;

    std::fs::write(&hash_cache, &current_hash)?;

    Ok(())
}
