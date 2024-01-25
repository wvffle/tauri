// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  collections::BTreeMap,
  fs::{copy, create_dir_all, File},
  io::{BufWriter, Write},
  path::PathBuf,
};

use anyhow::{Context, Result};
use schemars::{
  schema::{InstanceType, Metadata, RootSchema, Schema, SchemaObject, SubschemaValidation},
  schema_for,
};
use tauri_utils::{
  acl::{build::CapabilityFile, capability::Capability, plugin::Manifest},
  platform::Target,
};

const CAPABILITIES_SCHEMA_FILE_NAME: &str = "schema.json";
const CAPABILITIES_SCHEMA_FOLDER_NAME: &str = "schemas";

fn capabilities_schema(plugin_manifests: &BTreeMap<String, Manifest>) -> RootSchema {
  let mut schema = schema_for!(CapabilityFile);

  fn schema_from(plugin: &str, id: &str, description: Option<&str>) -> Schema {
    Schema::Object(SchemaObject {
      metadata: Some(Box::new(Metadata {
        description: description
          .as_ref()
          .map(|d| format!("{plugin}:{id} -> {d}")),
        ..Default::default()
      })),
      instance_type: Some(InstanceType::String.into()),
      enum_values: Some(vec![serde_json::Value::String(format!("{plugin}:{id}"))]),
      ..Default::default()
    })
  }

  let mut permission_schemas = Vec::new();

  for (plugin, manifest) in plugin_manifests {
    for (set_id, set) in &manifest.permission_sets {
      permission_schemas.push(schema_from(plugin, set_id, Some(&set.description)));
    }

    if let Some(default) = &manifest.default_permission {
      permission_schemas.push(schema_from(
        plugin,
        "default",
        Some(default.description.as_ref()),
      ));
    }

    for (permission_id, permission) in &manifest.permissions {
      permission_schemas.push(schema_from(
        plugin,
        permission_id,
        permission.description.as_deref(),
      ));
    }
  }

  if let Some(Schema::Object(obj)) = schema.definitions.get_mut("Identifier") {
    obj.object = None;
    obj.instance_type = None;
    obj.metadata.as_mut().map(|metadata| {
      metadata
        .description
        .replace("Permission identifier".to_string());
      metadata
    });
    obj.subschemas.replace(Box::new(SubschemaValidation {
      one_of: Some(permission_schemas),
      ..Default::default()
    }));
  }

  schema
}

pub fn generate_schema(
  plugin_manifests: &BTreeMap<String, Manifest>,
  target: Target,
) -> Result<()> {
  let schema = capabilities_schema(plugin_manifests);
  let schema_str = serde_json::to_string_pretty(&schema).unwrap();
  let out_dir = PathBuf::from("capabilities").join(CAPABILITIES_SCHEMA_FOLDER_NAME);
  create_dir_all(&out_dir).context("unable to create schema output directory")?;

  let schema_path = out_dir.join(format!("{target}-{CAPABILITIES_SCHEMA_FILE_NAME}"));
  let mut schema_file = BufWriter::new(File::create(&schema_path)?);
  write!(schema_file, "{schema_str}")?;

  copy(
    schema_path,
    out_dir.join(format!(
      "{}-{CAPABILITIES_SCHEMA_FILE_NAME}",
      if target.is_desktop() {
        "desktop"
      } else {
        "mobile"
      }
    )),
  )?;

  Ok(())
}

pub fn get_plugin_manifests() -> Result<BTreeMap<String, Manifest>> {
  let permission_map =
    tauri_utils::acl::build::read_permissions().context("failed to read plugin permissions")?;

  let mut processed = BTreeMap::new();
  for (plugin_name, permission_files) in permission_map {
    processed.insert(plugin_name, Manifest::from_files(permission_files));
  }

  Ok(processed)
}

pub fn validate_capabilities(
  plugin_manifests: &BTreeMap<String, Manifest>,
  capabilities: &BTreeMap<String, Capability>,
) -> Result<()> {
  let target = tauri_utils::platform::Target::from_triple(&std::env::var("TARGET").unwrap());

  for capability in capabilities.values() {
    if !capability.platforms.contains(&target) {
      continue;
    }

    for permission in &capability.permissions {
      if let Some((plugin_name, permission_name)) = permission.get().split_once(':') {
        let permission_exists = plugin_manifests
          .get(plugin_name)
          .map(|manifest| {
            if permission_name == "default" {
              manifest.default_permission.is_some()
            } else {
              manifest.permissions.contains_key(permission_name)
                || manifest.permission_sets.contains_key(permission_name)
            }
          })
          .unwrap_or(false);

        if !permission_exists {
          let mut available_permissions = Vec::new();
          for (plugin, manifest) in plugin_manifests {
            if manifest.default_permission.is_some() {
              available_permissions.push(format!("{plugin}:default"));
            }
            for p in manifest.permissions.keys() {
              available_permissions.push(format!("{plugin}:{p}"));
            }
            for p in manifest.permission_sets.keys() {
              available_permissions.push(format!("{plugin}:{p}"));
            }
          }

          anyhow::bail!(
            "Permission {} not found, expected one of {}",
            permission.get(),
            available_permissions.join(", ")
          );
        }
      }
    }
  }

  Ok(())
}
