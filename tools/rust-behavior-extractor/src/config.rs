//! Hash Cargo's invocation-directory hierarchy without exporting config secrets.
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn capture(
    root: &Path,
    cargo_home: Option<&Path>,
    environment: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for (depth, directory) in root.ancestors().enumerate() {
        capture_directory(
            &directory.join(".cargo"),
            &format!("cargo_ancestor_{depth}"),
            &mut result,
        )?;
    }
    if let Some(directory) = cargo_home {
        capture_directory(directory, "cargo_home", &mut result)?;
    }
    for (key, value) in environment {
        // Capture compilation-affecting Cargo overrides, never authentication,
        // registry, proxy or arbitrary build-script environment secrets.
        if matches!(
            key.as_str(),
            "RUSTFLAGS" | "CARGO_ENCODED_RUSTFLAGS" | "CARGO_BUILD_TARGET"
        ) || key.starts_with("CARGO_BUILD_")
            || (key.starts_with("CARGO_TARGET_") && key != "CARGO_TARGET_DIR")
            || key.starts_with("CARGO_PROFILE_")
            || key.starts_with("CARGO_UNSTABLE_")
        {
            result.insert(format!("environment:{key}:sha256"), hash(value.as_bytes()));
        }
    }
    Ok(result)
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn capture_directory(
    directory: &Path,
    label: &str,
    result: &mut BTreeMap<String, String>,
) -> Result<()> {
    let mut selected = None;
    // Cargo selects the extensionless legacy file when both exist. Hash both,
    // but traverse includes only in the file Cargo actually reads.
    for filename in ["config", "config.toml"] {
        let path = directory.join(filename);
        if path.is_file() {
            result.insert(
                format!("{label}:{filename}:sha256"),
                hash(&fs::read(&path)?),
            );
            if selected.is_none() {
                selected = Some(path);
            }
        }
    }
    if let Some(path) = selected {
        capture_includes(&path, label, result, &mut BTreeSet::new())?;
    }
    Ok(())
}

fn capture_includes(
    path: &Path,
    label: &str,
    result: &mut BTreeMap<String, String>,
    ancestors: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let canonical = fs::canonicalize(path)?;
    if !ancestors.insert(canonical.clone()) {
        return Err("Cargo configuration include cycle".into());
    }
    let content = fs::read_to_string(path)?;
    let config: toml::Value = toml::from_str(&content)?;
    if let Some(build) = config.get("build").and_then(toml::Value::as_table) {
        if ["rustc", "rustc-wrapper", "rustc-workspace-wrapper"]
            .iter()
            .any(|key| build.contains_key(*key))
        {
            return Err(
                "Cargo compiler/wrapper configuration overrides the pinned extractor".into(),
            );
        }
    }
    if let Some(includes) = config.get("include") {
        let includes = includes
            .as_array()
            .ok_or("unsupported Cargo include configuration")?;
        for (index, include) in includes.iter().enumerate() {
            let (relative, optional) = if let Some(path) = include.as_str() {
                (path, false)
            } else if let Some(table) = include.as_table() {
                if table.keys().any(|key| key != "path" && key != "optional") {
                    return Err("unsupported Cargo include options".into());
                }
                (
                    table
                        .get("path")
                        .and_then(toml::Value::as_str)
                        .ok_or("Cargo include lacks a path")?,
                    table
                        .get("optional")
                        .map(|value| {
                            value
                                .as_bool()
                                .ok_or("Cargo include optional must be Boolean")
                        })
                        .transpose()?
                        .unwrap_or(false),
                )
            } else {
                return Err("unsupported Cargo include entry".into());
            };
            let included = path
                .parent()
                .ok_or("Cargo config lacks a parent")?
                .join(relative);
            let key = format!("{label}:include_{index}");
            if !included.exists() && optional {
                result.insert(format!("{key}:sha256"), "absent".into());
                continue;
            }
            result.insert(format!("{key}:sha256"), hash(&fs::read(&included)?));
            capture_includes(&included, &key, result, ancestors)?;
        }
    }
    ancestors.remove(&canonical);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashes_parent_home_and_recursive_includes_without_paths_or_secrets() {
        let sandbox = std::env::temp_dir().join(format!("kr0ki-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sandbox);
        let root = sandbox.join("parent/workspace");
        let home = sandbox.join("cargo-home");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(sandbox.join("parent/.cargo")).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::write(
            sandbox.join("parent/.cargo/config.toml"),
            "include = ['flags.toml']\n",
        )
        .unwrap();
        fs::write(
            sandbox.join("parent/.cargo/flags.toml"),
            "[build]\nrustflags = ['--cfg', 'parent_flag']\n",
        )
        .unwrap();
        fs::write(
            home.join("config.toml"),
            "[registry]\ntoken = 'sensitive-test-value'\n",
        )
        .unwrap();
        let mut environment = BTreeMap::new();
        environment.insert(
            "CARGO_BUILD_RUSTFLAGS".into(),
            "--cfg environment_flag".into(),
        );
        environment.insert("CARGO_REGISTRY_TOKEN".into(), "secret-token".into());
        environment.insert("CARGO_TARGET_DIR".into(), "/random/first/target".into());
        let first = capture(&root, Some(&home), &environment).unwrap();
        assert!(first.contains_key("cargo_ancestor_1:config.toml:sha256"));
        assert!(first.contains_key("cargo_ancestor_1:include_0:sha256"));
        assert!(first.contains_key("cargo_home:config.toml:sha256"));
        assert!(first.contains_key("environment:CARGO_BUILD_RUSTFLAGS:sha256"));
        let serialized = serde_json::to_string(&first).unwrap();
        assert!(!serialized.contains("sensitive-test-value"));
        assert!(!serialized.contains("secret-token"));
        assert!(!serialized.contains(sandbox.to_str().unwrap()));
        environment.insert("CARGO_TARGET_DIR".into(), "/random/second/target".into());
        assert_eq!(first, capture(&root, Some(&home), &environment).unwrap());
        fs::write(
            sandbox.join("parent/.cargo/flags.toml"),
            "[build]\nrustflags = ['--cfg', 'changed']\n",
        )
        .unwrap();
        assert_ne!(first, capture(&root, Some(&home), &environment).unwrap());
        fs::write(home.join("config"), "include = ['cycle.toml']\n").unwrap();
        fs::write(home.join("cycle.toml"), "include = ['config']\n").unwrap();
        assert!(capture(&root, Some(&home), &environment).is_err());
        fs::write(
            home.join("config"),
            "[build]\nrustc-wrapper = 'different-compiler'\n",
        )
        .unwrap();
        assert!(capture(&root, Some(&home), &environment).is_err());
        fs::remove_dir_all(sandbox).unwrap();
    }
}
