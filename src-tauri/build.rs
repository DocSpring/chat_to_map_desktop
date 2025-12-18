use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Only run tauri_build when building the desktop app
    if env::var("CARGO_FEATURE_DESKTOP").is_ok() {
        tauri_build::build();
    }

    // Generate API types from OpenAPI spec
    generate_api_types();
}

fn generate_api_types() {
    let openapi_path = Path::new("openapi.json");

    // Tell Cargo to rerun if the OpenAPI spec changes
    println!("cargo:rerun-if-changed=openapi.json");

    if !openapi_path.exists() {
        // If openapi.json doesn't exist, skip generation
        println!("cargo:warning=openapi.json not found, skipping API type generation");
        return;
    }

    let spec_content = fs::read_to_string(openapi_path).expect("Failed to read openapi.json");
    let mut spec: openapiv3::OpenAPI =
        serde_json::from_str(&spec_content).expect("Failed to parse openapi.json");

    // Filter to only the endpoints the desktop app needs
    filter_endpoints(&mut spec);

    // Add operation IDs to all endpoints (required by progenitor)
    add_operation_ids(&mut spec);

    let mut generator_settings = progenitor::GenerationSettings::default();
    generator_settings.with_interface(progenitor::InterfaceStyle::Builder);
    generator_settings.with_tag(progenitor::TagStyle::Separate);

    let mut generator = progenitor::Generator::new(&generator_settings);

    let tokens = generator
        .generate_tokens(&spec)
        .expect("Failed to generate API client");

    let ast = syn::parse2(tokens).expect("Failed to parse generated code");
    let content = prettyplease::unparse(&ast);

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("chattomap_api.rs");

    fs::write(&out_path, content).expect("Failed to write generated API types");
}

/// Filter to only include endpoints the desktop app needs
fn filter_endpoints(spec: &mut openapiv3::OpenAPI) {
    // Only keep these paths
    let allowed_paths = ["/api/upload/presign", "/api/upload/complete"];

    spec.paths
        .paths
        .retain(|path, _| allowed_paths.contains(&path.as_str()));
}

/// Add operation IDs to all endpoints that don't have them
fn add_operation_ids(spec: &mut openapiv3::OpenAPI) {
    for (path, path_item) in spec.paths.paths.iter_mut() {
        if let openapiv3::ReferenceOr::Item(item) = path_item {
            add_operation_id_to_op(&mut item.get, path, "get");
            add_operation_id_to_op(&mut item.post, path, "post");
            add_operation_id_to_op(&mut item.put, path, "put");
            add_operation_id_to_op(&mut item.delete, path, "delete");
            add_operation_id_to_op(&mut item.patch, path, "patch");
        }
    }
}

fn add_operation_id_to_op(op: &mut Option<openapiv3::Operation>, path: &str, _method: &str) {
    if let Some(operation) = op {
        if operation.operation_id.is_none() {
            // Generate operation ID from path and method
            // e.g., "/api/upload/presign" + "post" -> "upload_presign"
            let id = path
                .trim_start_matches("/api/")
                .replace('/', "_")
                .replace(['{', '}'], "");
            // Only add method suffix if there are multiple methods
            operation.operation_id = Some(id);
        }
    }
}
