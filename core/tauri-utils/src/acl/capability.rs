// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! End-user abstraction for selecting permissions a window has access to.

use crate::{acl::Identifier, platform::Target};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]

/// A set of direct capabilities grouped together under a new name.
pub struct CapabilitySet {
  inner: Vec<Capability>,
}

/// a grouping and boundary mechanism developers can use to separate windows or plugins functionality from each other at runtime.
///
/// If a window is not matching any capability then it has no access to the IPC layer at all.
///
/// This can be done to create trust groups and reduce impact of vulnerabilities in certain plugins or windows.
/// Windows can be added to a capability by exact name or glob patterns like *, admin-* or main-window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Capability {
  /// Identifier of the capability.
  pub identifier: String,
  /// Description of the capability.
  #[serde(default)]
  pub description: String,
  /// Execution context of the capability.
  ///
  /// At runtime, Tauri filters the IPC command together with the context to determine wheter it is allowed or not and its scope.
  #[serde(default)]
  pub context: CapabilityContext,
  /// List of windows that uses this capability. Can be a glob pattern.
  pub windows: Vec<String>,
  /// List of permissions attached to this capability. Must include the plugin name as prefix in the form of `${plugin-name}:${permission-name}`.
  pub permissions: Vec<Identifier>,
  /// Target platforms this capability applies. By default all platforms applies.
  #[serde(default = "default_platforms")]
  pub platforms: Vec<Target>,
}

fn default_platforms() -> Vec<Target> {
  vec![
    Target::Linux,
    Target::MacOS,
    Target::Windows,
    Target::Android,
    Target::Ios,
  ]
}

/// Context of the capability.
#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum CapabilityContext {
  /// Capability refers to local URL usage.
  #[default]
  Local,
  /// Capability refers to remote usage.
  Remote {
    /// Remote domains this capability refers to. Can use glob patterns.
    domains: Vec<String>,
  },
}
