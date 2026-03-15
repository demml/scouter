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
    // stored next to the generated file so it is shared across all compilation
    // contexts (OUT_DIR is per-feature-set and ephemeral, causing spurious
    // regeneration when scouter_tonic is compiled as a transitive dep with a
    // different feature set).
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let hash_cache = PathBuf::from("src/generated/.proto_hash");
    let generated = PathBuf::from("src/generated/scouter.grpc.v1.rs");
    // Hash only proto content — feature flags are intentionally excluded.
    // The generated file always includes both server and client code; the
    // #[cfg(feature = "...")] guards in lib.rs handle conditional exposure.
    // Including features in the hash caused spurious regeneration whenever
    // scouter_tonic compiled as a transitive dep with a different feature set.
    let current_hash = hash_protos(protos).to_string();

    if generated.exists() {
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

    let descriptor_path = out_dir.join("scouter_descriptor.bin");

    // Always generate with both server and client so the file is identical
    // regardless of which features are active. lib.rs uses #[cfg(feature)]
    // to gate what is re-exported.
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
