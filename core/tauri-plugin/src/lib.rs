// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! [![](https://github.com/tauri-apps/tauri/raw/dev/.github/splash.png)](https://tauri.app)
//!
//! Interface for building Tauri plugins.

#![doc(
  html_logo_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png",
  html_favicon_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "build")]
mod build;
#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "build")]
#[cfg_attr(docsrs, doc(feature = "build"))]
pub use build::*;
#[cfg(feature = "runtime")]
#[cfg_attr(docsrs, doc(feature = "runtime"))]
pub use runtime::*;
