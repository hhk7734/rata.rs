use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn load_and_resolve(
    path: impl AsRef<Path>,
) -> Result<(openapiv3::OpenAPI, Value, Vec<PathBuf>)> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let mut value: Value = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML from {}", path.display()))?;

    let mut tracked_files = vec![path.to_path_buf()];
    resolve_refs(
        &mut value,
        path.parent().unwrap_or(Path::new(".")),
        &mut tracked_files,
    )?;

    let openapi: openapiv3::OpenAPI = serde_json::from_value(value.clone())
        .with_context(|| "Failed to deserialize resolved OpenAPI spec")?;

    Ok((openapi, value, tracked_files))
}

fn resolve_refs(
    value: &mut Value,
    base_dir: &Path,
    tracked_files: &mut Vec<PathBuf>,
) -> Result<()> {
    match value {
        Value::Object(map) => {
            if let Some(ref_str) = map.get("$ref").and_then(|v| v.as_str()) {
                // Only resolve external local files for now
                if !ref_str.starts_with('#') && !ref_str.starts_with("http") {
                    let (file_path, json_ptr) = if let Some(idx) = ref_str.find('#') {
                        (&ref_str[..idx], Some(&ref_str[idx + 1..]))
                    } else {
                        (ref_str, None)
                    };

                    let target_path = base_dir.join(file_path);
                    tracked_files.push(target_path.clone());
                    let content = std::fs::read_to_string(&target_path).with_context(|| {
                        format!("Failed to read referenced file {}", target_path.display())
                    })?;

                    let mut resolved_val: Value =
                        serde_yaml::from_str(&content).with_context(|| {
                            format!("Failed to parse YAML from {}", target_path.display())
                        })?;

                    // Recursively resolve references in the loaded file
                    resolve_refs(
                        &mut resolved_val,
                        target_path.parent().unwrap_or(Path::new(".")),
                        tracked_files,
                    )?;

                    if let Some(ptr) = json_ptr {
                        let mut current = &resolved_val;
                        if !ptr.is_empty() && ptr != "/" {
                            for token in ptr.trim_start_matches('/').split('/') {
                                let token = token.replace("~1", "/").replace("~0", "~");
                                current = match current {
                                    Value::Object(m) => m.get(&token).with_context(|| {
                                        format!("Token {} not found in object", token)
                                    })?,
                                    Value::Array(a) => {
                                        let idx: usize = token.parse().with_context(|| {
                                            format!("Invalid array index {}", token)
                                        })?;
                                        a.get(idx).with_context(|| {
                                            format!("Index {} out of bounds", idx)
                                        })?
                                    }
                                    _ => anyhow::bail!(
                                        "Cannot traverse token {} in non-object/array",
                                        token
                                    ),
                                };
                            }
                        }
                        *value = current.clone();
                    } else {
                        *value = resolved_val;
                    }
                    return Ok(());
                }
            }

            // Recurse into children
            for v in map.values_mut() {
                resolve_refs(v, base_dir, tracked_files)?;
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                resolve_refs(v, base_dir, tracked_files)?;
            }
        }
        _ => {}
    }
    Ok(())
}
