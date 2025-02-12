// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::path::Path;

use cargo_metadata::{Metadata, MetadataCommand};
use tauri::utils::acl::{self, Error};

pub struct Builder<'a> {
  commands: &'a [&'static str],
}

impl<'a> Builder<'a> {
  pub fn new(commands: &'a [&'static str]) -> Self {
    Self { commands }
  }

  /// [`Self::try_build`] but will exit automatically if an error is found.
  pub fn build(self) {
    if let Err(error) = self.try_build() {
      println!("{}: {}", env!("CARGO_PKG_NAME"), error);
      std::process::exit(1);
    }
  }

  /// Ensure this crate is properly configured to be a Tauri plugin.
  ///
  /// # Errors
  ///
  /// Errors will occur if environmental variables expected to be set inside of [build scripts]
  /// are not found, or if the crate violates Tauri plugin conventions.
  pub fn try_build(self) -> Result<(), Error> {
    // convention: plugin names should not use underscores
    let name = build_var("CARGO_PKG_NAME")?;
    if name.contains('_') {
      return Err(Error::CrateName);
    }

    // requirement: links MUST be set and MUST match the name
    let _links = build_var("CARGO_MANIFEST_LINKS")?;

    let autogenerated = Path::new("permissions/autogenerated/");
    let commands_dir = &autogenerated.join("commands");

    if !self.commands.is_empty() {
      acl::build::autogenerate_command_permissions(commands_dir, self.commands, "");
    }

    let permissions = acl::build::define_permissions("./permissions/**/*.*", &name)?;
    acl::build::generate_schema(&permissions, "./permissions")?;

    let metadata = find_metadata()?;
    println!("{metadata:#?}");

    Ok(())
  }
}

/// Grab an env var that is expected to be set inside of build scripts.
fn build_var(key: &'static str) -> Result<String, Error> {
  std::env::var(key).map_err(|_| Error::BuildVar(key))
}

fn find_metadata() -> Result<Metadata, Error> {
  build_var("CARGO_MANIFEST_DIR").and_then(|dir| {
    MetadataCommand::new()
      .current_dir(dir)
      .no_deps()
      .exec()
      .map_err(Error::Metadata)
  })
}
