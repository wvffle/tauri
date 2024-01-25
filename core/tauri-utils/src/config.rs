// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! The Tauri configuration used at runtime.
//!
//! It is pulled from a `tauri.conf.json` file and the [`Config`] struct is generated at compile time.
//!
//! # Stability
//! This is a core functionality that is not considered part of the stable API.
//! If you use it, note that it may include breaking changes in the future.

#[cfg(target_os = "linux")]
use heck::ToKebabCase;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use semver::Version;
use serde::{
  de::{Deserializer, Error as DeError, Visitor},
  Deserialize, Serialize, Serializer,
};
use serde_json::Value as JsonValue;
use serde_with::skip_serializing_none;
use url::Url;

use std::{
  collections::HashMap,
  fmt::{self, Display},
  fs::read_to_string,
  path::PathBuf,
  str::FromStr,
};

/// Items to help with parsing content into a [`Config`].
pub mod parse;

use crate::{TitleBarStyle, WindowEffect, WindowEffectState};

pub use self::parse::parse;

fn default_true() -> bool {
  true
}

/// An URL to open on a Tauri webview window.
#[derive(PartialEq, Eq, Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
#[non_exhaustive]
pub enum WebviewUrl {
  /// An external URL.
  External(Url),
  /// The path portion of an app URL.
  /// For instance, to load `tauri://localhost/users/john`,
  /// you can simply provide `users/john` in this configuration.
  App(PathBuf),
}

impl fmt::Display for WebviewUrl {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::External(url) => write!(f, "{url}"),
      Self::App(path) => write!(f, "{}", path.display()),
    }
  }
}

impl Default for WebviewUrl {
  fn default() -> Self {
    Self::App("index.html".into())
  }
}

/// A bundle referenced by tauri-bundler.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename_all = "lowercase"))]
pub enum BundleType {
  /// The debian bundle (.deb).
  Deb,
  /// The RPM bundle (.rpm).
  Rpm,
  /// The AppImage bundle (.appimage).
  AppImage,
  /// The Microsoft Installer bundle (.msi).
  Msi,
  /// The NSIS bundle (.exe).
  Nsis,
  /// The macOS application bundle (.app).
  App,
  /// The Apple Disk Image bundle (.dmg).
  Dmg,
  /// The Tauri updater bundle.
  Updater,
}

impl Display for BundleType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}",
      match self {
        Self::Deb => "deb",
        Self::Rpm => "rpm",
        Self::AppImage => "appimage",
        Self::Msi => "msi",
        Self::Nsis => "nsis",
        Self::App => "app",
        Self::Dmg => "dmg",
        Self::Updater => "updater",
      }
    )
  }
}

impl Serialize for BundleType {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(self.to_string().as_ref())
  }
}

impl<'de> Deserialize<'de> for BundleType {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    match s.to_lowercase().as_str() {
      "deb" => Ok(Self::Deb),
      "rpm" => Ok(Self::Rpm),
      "appimage" => Ok(Self::AppImage),
      "msi" => Ok(Self::Msi),
      "nsis" => Ok(Self::Nsis),
      "app" => Ok(Self::App),
      "dmg" => Ok(Self::Dmg),
      "updater" => Ok(Self::Updater),
      _ => Err(DeError::custom(format!("unknown bundle target '{s}'"))),
    }
  }
}

/// Targets to bundle. Each value is case insensitive.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BundleTarget {
  /// Bundle all targets.
  All,
  /// A list of bundle targets.
  List(Vec<BundleType>),
  /// A single bundle target.
  One(BundleType),
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for BundleTarget {
  fn schema_name() -> std::string::String {
    "BundleTarget".to_owned()
  }

  fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    let any_of = vec![
      schemars::schema::SchemaObject {
        enum_values: Some(vec!["all".into()]),
        metadata: Some(Box::new(schemars::schema::Metadata {
          description: Some("Bundle all targets.".to_owned()),
          ..Default::default()
        })),
        ..Default::default()
      }
      .into(),
      schemars::_private::apply_metadata(
        gen.subschema_for::<Vec<BundleType>>(),
        schemars::schema::Metadata {
          description: Some("A list of bundle targets.".to_owned()),
          ..Default::default()
        },
      ),
      schemars::_private::apply_metadata(
        gen.subschema_for::<BundleType>(),
        schemars::schema::Metadata {
          description: Some("A single bundle target.".to_owned()),
          ..Default::default()
        },
      ),
    ];

    schemars::schema::SchemaObject {
      subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
        any_of: Some(any_of),
        ..Default::default()
      })),
      metadata: Some(Box::new(schemars::schema::Metadata {
        description: Some("Targets to bundle. Each value is case insensitive.".to_owned()),
        ..Default::default()
      })),
      ..Default::default()
    }
    .into()
  }
}

impl Default for BundleTarget {
  fn default() -> Self {
    Self::All
  }
}

impl Serialize for BundleTarget {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    match self {
      Self::All => serializer.serialize_str("all"),
      Self::List(l) => l.serialize(serializer),
      Self::One(t) => serializer.serialize_str(t.to_string().as_ref()),
    }
  }
}

impl<'de> Deserialize<'de> for BundleTarget {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    #[derive(Deserialize, Serialize)]
    #[serde(untagged)]
    pub enum BundleTargetInner {
      List(Vec<BundleType>),
      One(BundleType),
      All(String),
    }

    match BundleTargetInner::deserialize(deserializer)? {
      BundleTargetInner::All(s) if s.to_lowercase() == "all" => Ok(Self::All),
      BundleTargetInner::All(t) => Err(DeError::custom(format!("invalid bundle type {t}"))),
      BundleTargetInner::List(l) => Ok(Self::List(l)),
      BundleTargetInner::One(t) => Ok(Self::One(t)),
    }
  }
}

impl BundleTarget {
  /// Gets the bundle targets as a [`Vec`]. The vector is empty when set to [`BundleTarget::All`].
  #[allow(dead_code)]
  pub fn to_vec(&self) -> Vec<BundleType> {
    match self {
      Self::All => vec![],
      Self::List(list) => list.clone(),
      Self::One(i) => vec![i.clone()],
    }
  }
}

/// Configuration for AppImage bundles.
///
/// See more: <https://tauri.app/v1/api/config#appimageconfig>
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppImageConfig {
  /// Include additional gstreamer dependencies needed for audio and video playback.
  /// This increases the bundle size by ~15-35MB depending on your build system.
  #[serde(default, alias = "bundle-media-framework")]
  pub bundle_media_framework: bool,
}

/// Configuration for Debian (.deb) bundles.
///
/// See more: <https://tauri.app/v1/api/config#debconfig>
#[skip_serializing_none]
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DebConfig {
  /// The list of deb dependencies your application relies on.
  pub depends: Option<Vec<String>>,
  /// The files to include on the package.
  #[serde(default)]
  pub files: HashMap<PathBuf, PathBuf>,
  /// Path to a custom desktop file Handlebars template.
  ///
  /// Available variables: `categories`, `comment` (optional), `exec`, `icon` and `name`.
  pub desktop_template: Option<PathBuf>,
}

/// Configuration for RPM bundles.
#[skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpmConfig {
  /// The package's license identifier. If not set, defaults to the license from
  /// the Cargo.toml file.
  pub license: Option<String>,
  /// The list of RPM dependencies your application relies on.
  pub depends: Option<Vec<String>>,
  /// The RPM release tag.
  #[serde(default = "default_release")]
  pub release: String,
  /// The RPM epoch.
  #[serde(default)]
  pub epoch: u32,
  /// The files to include on the package.
  #[serde(default)]
  pub files: HashMap<PathBuf, PathBuf>,
  /// Path to a custom desktop file Handlebars template.
  ///
  /// Available variables: `categories`, `comment` (optional), `exec`, `icon` and `name`.
  pub desktop_template: Option<PathBuf>,
}

impl Default for RpmConfig {
  fn default() -> Self {
    Self {
      license: None,
      depends: None,
      release: default_release(),
      epoch: 0,
      files: Default::default(),
      desktop_template: None,
    }
  }
}

fn default_release() -> String {
  "1".into()
}

/// Position coordinates struct.
#[derive(Default, Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Position {
  /// X coordinate.
  pub x: u32,
  /// Y coordinate.
  pub y: u32,
}

/// Size of the window.
#[derive(Default, Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Size {
  /// Width of the window.
  pub width: u32,
  /// Height of the window.
  pub height: u32,
}

/// Configuration for Apple Disk Image (.dmg) bundles.
///
/// See more: <https://tauri.app/v1/api/config#dmgconfig>
#[skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DmgConfig {
  /// Image to use as the background in dmg file. Accepted formats: `png`/`jpg`/`gif`.
  pub background: Option<PathBuf>,
  /// Position of volume window on screen.
  pub window_position: Option<Position>,
  /// Size of volume window.
  #[serde(default = "dmg_window_size", alias = "window-size")]
  pub window_size: Size,
  /// Position of app file on window.
  #[serde(default = "dmg_app_position", alias = "app-position")]
  pub app_position: Position,
  /// Position of application folder on window.
  #[serde(
    default = "dmg_application_folder_position",
    alias = "application-folder-position"
  )]
  pub application_folder_position: Position,
}

impl Default for DmgConfig {
  fn default() -> Self {
    Self {
      background: None,
      window_position: None,
      window_size: dmg_window_size(),
      app_position: dmg_app_position(),
      application_folder_position: dmg_application_folder_position(),
    }
  }
}

fn dmg_window_size() -> Size {
  Size {
    width: 660,
    height: 400,
  }
}

fn dmg_app_position() -> Position {
  Position { x: 180, y: 170 }
}

fn dmg_application_folder_position() -> Position {
  Position { x: 480, y: 170 }
}

fn de_minimum_system_version<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
  D: Deserializer<'de>,
{
  let version = Option::<String>::deserialize(deserializer)?;
  match version {
    Some(v) if v.is_empty() => Ok(minimum_system_version()),
    e => Ok(e),
  }
}

/// Configuration for the macOS bundles.
///
/// See more: <https://tauri.app/v1/api/config#macconfig>
#[skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MacConfig {
  /// A list of strings indicating any macOS X frameworks that need to be bundled with the application.
  ///
  /// If a name is used, ".framework" must be omitted and it will look for standard install locations. You may also use a path to a specific framework.
  pub frameworks: Option<Vec<String>>,
  /// The files to include in the application relative to the Contents directory.
  #[serde(default)]
  pub files: HashMap<PathBuf, PathBuf>,
  /// A version string indicating the minimum macOS X version that the bundled application supports. Defaults to `10.13`.
  ///
  /// Setting it to `null` completely removes the `LSMinimumSystemVersion` field on the bundle's `Info.plist`
  /// and the `MACOSX_DEPLOYMENT_TARGET` environment variable.
  ///
  /// An empty string is considered an invalid value so the default value is used.
  #[serde(
    deserialize_with = "de_minimum_system_version",
    default = "minimum_system_version",
    alias = "minimum-system-version"
  )]
  pub minimum_system_version: Option<String>,
  /// Allows your application to communicate with the outside world.
  /// It should be a lowercase, without port and protocol domain name.
  #[serde(alias = "exception-domain")]
  pub exception_domain: Option<String>,
  /// The path to the license file to add to the DMG bundle.
  pub license: Option<String>,
  /// Identity to use for code signing.
  #[serde(alias = "signing-identity")]
  pub signing_identity: Option<String>,
  /// Provider short name for notarization.
  #[serde(alias = "provider-short-name")]
  pub provider_short_name: Option<String>,
  /// Path to the entitlements file.
  pub entitlements: Option<String>,
}

impl Default for MacConfig {
  fn default() -> Self {
    Self {
      frameworks: None,
      files: HashMap::new(),
      minimum_system_version: minimum_system_version(),
      exception_domain: None,
      license: None,
      signing_identity: None,
      provider_short_name: None,
      entitlements: None,
    }
  }
}

fn minimum_system_version() -> Option<String> {
  Some("10.13".into())
}

/// Configuration for a target language for the WiX build.
///
/// See more: <https://tauri.app/v1/api/config#wixlanguageconfig>
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WixLanguageConfig {
  /// The path to a locale (`.wxl`) file. See <https://wixtoolset.org/documentation/manual/v3/howtos/ui_and_localization/build_a_localized_version.html>.
  #[serde(alias = "locale-path")]
  pub locale_path: Option<String>,
}

/// The languages to build using WiX.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum WixLanguage {
  /// A single language to build, without configuration.
  One(String),
  /// A list of languages to build, without configuration.
  List(Vec<String>),
  /// A map of languages and its configuration.
  Localized(HashMap<String, WixLanguageConfig>),
}

impl Default for WixLanguage {
  fn default() -> Self {
    Self::One("en-US".into())
  }
}

/// Configuration for the MSI bundle using WiX.
///
/// See more: <https://tauri.app/v1/api/config#wixconfig>
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WixConfig {
  /// The installer languages to build. See <https://docs.microsoft.com/en-us/windows/win32/msi/localizing-the-error-and-actiontext-tables>.
  #[serde(default)]
  pub language: WixLanguage,
  /// A custom .wxs template to use.
  pub template: Option<PathBuf>,
  /// A list of paths to .wxs files with WiX fragments to use.
  #[serde(default, alias = "fragment-paths")]
  pub fragment_paths: Vec<PathBuf>,
  /// The ComponentGroup element ids you want to reference from the fragments.
  #[serde(default, alias = "component-group-refs")]
  pub component_group_refs: Vec<String>,
  /// The Component element ids you want to reference from the fragments.
  #[serde(default, alias = "component-refs")]
  pub component_refs: Vec<String>,
  /// The FeatureGroup element ids you want to reference from the fragments.
  #[serde(default, alias = "feature-group-refs")]
  pub feature_group_refs: Vec<String>,
  /// The Feature element ids you want to reference from the fragments.
  #[serde(default, alias = "feature-refs")]
  pub feature_refs: Vec<String>,
  /// The Merge element ids you want to reference from the fragments.
  #[serde(default, alias = "merge-refs")]
  pub merge_refs: Vec<String>,
  /// Disables the Webview2 runtime installation after app install.
  ///
  /// Will be removed in v2, prefer the [`WindowsConfig::webview_install_mode`] option.
  #[serde(default, alias = "skip-webview-install")]
  pub skip_webview_install: bool,
  /// The path to the license file to render on the installer.
  ///
  /// Must be an RTF file, so if a different extension is provided, we convert it to the RTF format.
  pub license: Option<PathBuf>,
  /// Create an elevated update task within Windows Task Scheduler.
  #[serde(default, alias = "enable-elevated-update-task")]
  pub enable_elevated_update_task: bool,
  /// Path to a bitmap file to use as the installation user interface banner.
  /// This bitmap will appear at the top of all but the first page of the installer.
  ///
  /// The required dimensions are 493px × 58px.
  #[serde(alias = "banner-path")]
  pub banner_path: Option<PathBuf>,
  /// Path to a bitmap file to use on the installation user interface dialogs.
  /// It is used on the welcome and completion dialogs.

  /// The required dimensions are 493px × 312px.
  #[serde(alias = "dialog-image-path")]
  pub dialog_image_path: Option<PathBuf>,
}

/// Compression algorithms used in the NSIS installer.
///
/// See <https://nsis.sourceforge.io/Reference/SetCompressor>
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum NsisCompression {
  /// ZLIB uses the deflate algorithm, it is a quick and simple method. With the default compression level it uses about 300 KB of memory.
  Zlib,
  /// BZIP2 usually gives better compression ratios than ZLIB, but it is a bit slower and uses more memory. With the default compression level it uses about 4 MB of memory.
  Bzip2,
  /// LZMA (default) is a new compression method that gives very good compression ratios. The decompression speed is high (10-20 MB/s on a 2 GHz CPU), the compression speed is lower. The memory size that will be used for decompression is the dictionary size plus a few KBs, the default is 8 MB.
  Lzma,
}

/// Configuration for the Installer bundle using NSIS.
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NsisConfig {
  /// A custom .nsi template to use.
  pub template: Option<PathBuf>,
  /// The path to the license file to render on the installer.
  pub license: Option<PathBuf>,
  /// The path to a bitmap file to display on the header of installers pages.
  ///
  /// The recommended dimensions are 150px x 57px.
  #[serde(alias = "header-image")]
  pub header_image: Option<PathBuf>,
  /// The path to a bitmap file for the Welcome page and the Finish page.
  ///
  /// The recommended dimensions are 164px x 314px.
  #[serde(alias = "sidebar-image")]
  pub sidebar_image: Option<PathBuf>,
  /// The path to an icon file used as the installer icon.
  #[serde(alias = "install-icon")]
  pub installer_icon: Option<PathBuf>,
  /// Whether the installation will be for all users or just the current user.
  #[serde(default, alias = "install-mode")]
  pub install_mode: NSISInstallerMode,
  /// A list of installer languages.
  /// By default the OS language is used. If the OS language is not in the list of languages, the first language will be used.
  /// To allow the user to select the language, set `display_language_selector` to `true`.
  ///
  /// See <https://github.com/kichik/nsis/tree/9465c08046f00ccb6eda985abbdbf52c275c6c4d/Contrib/Language%20files> for the complete list of languages.
  pub languages: Option<Vec<String>>,
  /// A key-value pair where the key is the language and the
  /// value is the path to a custom `.nsh` file that holds the translated text for tauri's custom messages.
  ///
  /// See <https://github.com/tauri-apps/tauri/blob/dev/tooling/bundler/src/bundle/windows/templates/nsis-languages/English.nsh> for an example `.nsh` file.
  ///
  /// **Note**: the key must be a valid NSIS language and it must be added to [`NsisConfig`] languages array,
  pub custom_language_files: Option<HashMap<String, PathBuf>>,
  /// Whether to display a language selector dialog before the installer and uninstaller windows are rendered or not.
  /// By default the OS language is selected, with a fallback to the first language in the `languages` array.
  #[serde(default, alias = "display-language-selector")]
  pub display_language_selector: bool,
  /// Set the compression algorithm used to compress files in the installer.
  ///
  /// See <https://nsis.sourceforge.io/Reference/SetCompressor>
  pub compression: Option<NsisCompression>,
}

/// Install Modes for the NSIS installer.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum NSISInstallerMode {
  /// Default mode for the installer.
  ///
  /// Install the app by default in a directory that doesn't require Administrator access.
  ///
  /// Installer metadata will be saved under the `HKCU` registry path.
  CurrentUser,
  /// Install the app by default in the `Program Files` folder directory requires Administrator
  /// access for the installation.
  ///
  /// Installer metadata will be saved under the `HKLM` registry path.
  PerMachine,
  /// Combines both modes and allows the user to choose at install time
  /// whether to install for the current user or per machine. Note that this mode
  /// will require Administrator access even if the user wants to install it for the current user only.
  ///
  /// Installer metadata will be saved under the `HKLM` or `HKCU` registry path based on the user's choice.
  Both,
}

impl Default for NSISInstallerMode {
  fn default() -> Self {
    Self::CurrentUser
  }
}

/// Install modes for the Webview2 runtime.
/// Note that for the updater bundle [`Self::DownloadBootstrapper`] is used.
///
/// For more information see <https://tauri.app/v1/guides/building/windows>.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum WebviewInstallMode {
  /// Do not install the Webview2 as part of the Windows Installer.
  Skip,
  /// Download the bootstrapper and run it.
  /// Requires an internet connection.
  /// Results in a smaller installer size, but is not recommended on Windows 7.
  DownloadBootstrapper {
    /// Instructs the installer to run the bootstrapper in silent mode. Defaults to `true`.
    #[serde(default = "default_true")]
    silent: bool,
  },
  /// Embed the bootstrapper and run it.
  /// Requires an internet connection.
  /// Increases the installer size by around 1.8MB, but offers better support on Windows 7.
  EmbedBootstrapper {
    /// Instructs the installer to run the bootstrapper in silent mode. Defaults to `true`.
    #[serde(default = "default_true")]
    silent: bool,
  },
  /// Embed the offline installer and run it.
  /// Does not require an internet connection.
  /// Increases the installer size by around 127MB.
  OfflineInstaller {
    /// Instructs the installer to run the installer in silent mode. Defaults to `true`.
    #[serde(default = "default_true")]
    silent: bool,
  },
  /// Embed a fixed webview2 version and use it at runtime.
  /// Increases the installer size by around 180MB.
  FixedRuntime {
    /// The path to the fixed runtime to use.
    ///
    /// The fixed version can be downloaded [on the official website](https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section).
    /// The `.cab` file must be extracted to a folder and this folder path must be defined on this field.
    path: PathBuf,
  },
}

impl Default for WebviewInstallMode {
  fn default() -> Self {
    Self::DownloadBootstrapper { silent: true }
  }
}

/// Windows bundler configuration.
///
/// See more: <https://tauri.app/v1/api/config#windowsconfig>
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowsConfig {
  /// Specifies the file digest algorithm to use for creating file signatures.
  /// Required for code signing. SHA-256 is recommended.
  #[serde(alias = "digest-algorithm")]
  pub digest_algorithm: Option<String>,
  /// Specifies the SHA1 hash of the signing certificate.
  #[serde(alias = "certificate-thumbprint")]
  pub certificate_thumbprint: Option<String>,
  /// Server to use during timestamping.
  #[serde(alias = "timestamp-url")]
  pub timestamp_url: Option<String>,
  /// Whether to use Time-Stamp Protocol (TSP, a.k.a. RFC 3161) for the timestamp server. Your code signing provider may
  /// use a TSP timestamp server, like e.g. SSL.com does. If so, enable TSP by setting to true.
  #[serde(default)]
  pub tsp: bool,
  /// The installation mode for the Webview2 runtime.
  #[serde(default, alias = "webview-install-mode")]
  pub webview_install_mode: WebviewInstallMode,
  /// Path to the webview fixed runtime to use. Overwrites [`Self::webview_install_mode`] if set.
  ///
  /// Will be removed in v2, prefer the [`Self::webview_install_mode`] option.
  ///
  /// The fixed version can be downloaded [on the official website](https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section).
  /// The `.cab` file must be extracted to a folder and this folder path must be defined on this field.
  #[serde(alias = "webview-fixed-runtime-path")]
  pub webview_fixed_runtime_path: Option<PathBuf>,
  /// Validates a second app installation, blocking the user from installing an older version if set to `false`.
  ///
  /// For instance, if `1.2.1` is installed, the user won't be able to install app version `1.2.0` or `1.1.5`.
  ///
  /// The default value of this flag is `true`.
  #[serde(default = "default_true", alias = "allow-downgrades")]
  pub allow_downgrades: bool,
  /// Configuration for the MSI generated with WiX.
  pub wix: Option<WixConfig>,
  /// Configuration for the installer generated with NSIS.
  pub nsis: Option<NsisConfig>,
}

impl Default for WindowsConfig {
  fn default() -> Self {
    Self {
      digest_algorithm: None,
      certificate_thumbprint: None,
      timestamp_url: None,
      tsp: false,
      webview_install_mode: Default::default(),
      webview_fixed_runtime_path: None,
      allow_downgrades: true,
      wix: None,
      nsis: None,
    }
  }
}

/// macOS-only. Corresponds to CFBundleTypeRole
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum BundleTypeRole {
  /// CFBundleTypeRole.Editor. Files can be read and edited.
  #[default]
  Editor,
  /// CFBundleTypeRole.Viewer. Files can be read.
  Viewer,
  /// CFBundleTypeRole.Shell
  Shell,
  /// CFBundleTypeRole.QLGenerator
  QLGenerator,
  /// CFBundleTypeRole.None
  None,
}

impl Display for BundleTypeRole {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Editor => write!(f, "Editor"),
      Self::Viewer => write!(f, "Viewer"),
      Self::Shell => write!(f, "Shell"),
      Self::QLGenerator => write!(f, "QLGenerator"),
      Self::None => write!(f, "None"),
    }
  }
}

/// An extension for a [`FileAssociation`].
///
/// A leading `.` is automatically stripped.
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct AssociationExt(pub String);

impl fmt::Display for AssociationExt {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl<'d> serde::Deserialize<'d> for AssociationExt {
  fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
    let ext = String::deserialize(deserializer)?;
    if let Some(ext) = ext.strip_prefix('.') {
      Ok(AssociationExt(ext.into()))
    } else {
      Ok(AssociationExt(ext))
    }
  }
}

/// File association
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileAssociation {
  /// File extensions to associate with this app. e.g. 'png'
  pub ext: Vec<AssociationExt>,
  /// The name. Maps to `CFBundleTypeName` on macOS. Default to `ext[0]`
  pub name: Option<String>,
  /// The association description. Windows-only. It is displayed on the `Type` column on Windows Explorer.
  pub description: Option<String>,
  /// The app’s role with respect to the type. Maps to `CFBundleTypeRole` on macOS.
  #[serde(default)]
  pub role: BundleTypeRole,
  /// The mime-type e.g. 'image/png' or 'text/plain'. Linux-only.
  #[serde(alias = "mime-type")]
  pub mime_type: Option<String>,
}

/// The Updater configuration object.
///
/// See more: <https://tauri.app/v1/api/config#updaterconfig>
#[skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterConfig {
  /// Whether the updater is active or not.
  #[serde(default)]
  pub active: bool,
  /// Signature public key.
  #[serde(default)] // use default just so the schema doesn't flag it as required
  pub pubkey: String,
  /// The Windows configuration for the updater.
  #[serde(default)]
  pub windows: UpdaterWindowsConfig,
}

impl<'de> Deserialize<'de> for UpdaterConfig {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    #[derive(Deserialize)]
    struct InnerUpdaterConfig {
      #[serde(default)]
      active: bool,
      pubkey: Option<String>,
      #[serde(default)]
      windows: UpdaterWindowsConfig,
    }

    let config = InnerUpdaterConfig::deserialize(deserializer)?;

    if config.active && config.pubkey.is_none() {
      return Err(DeError::custom(
        "The updater `pubkey` configuration is required.",
      ));
    }

    Ok(UpdaterConfig {
      active: config.active,
      pubkey: config.pubkey.unwrap_or_default(),
      windows: config.windows,
    })
  }
}

impl Default for UpdaterConfig {
  fn default() -> Self {
    Self {
      active: false,
      pubkey: "".into(),
      windows: Default::default(),
    }
  }
}

/// Definition for bundle resources.
/// Can be either a list of paths to include or a map of source to target paths.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields, untagged)]
pub enum BundleResources {
  /// A list of paths to include.
  List(Vec<String>),
  /// A map of source to target paths.
  Map(HashMap<String, String>),
}

impl BundleResources {
  /// Adds a path to the resource collection.
  pub fn push(&mut self, path: impl Into<String>) {
    match self {
      Self::List(l) => l.push(path.into()),
      Self::Map(l) => {
        let path = path.into();
        l.insert(path.clone(), path);
      }
    }
  }
}

/// Configuration for tauri-bundler.
///
/// See more: <https://tauri.app/v1/api/config#bundleconfig>
#[skip_serializing_none]
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleConfig {
  /// Whether Tauri should bundle your application or just output the executable.
  #[serde(default)]
  pub active: bool,
  /// The bundle targets, currently supports ["deb", "rpm", "appimage", "nsis", "msi", "app", "dmg", "updater"] or "all".
  #[serde(default)]
  pub targets: BundleTarget,
  /// The application identifier in reverse domain name notation (e.g. `com.tauri.example`).
  /// This string must be unique across applications since it is used in system configurations like
  /// the bundle ID and path to the webview data directory.
  /// This string must contain only alphanumeric characters (A–Z, a–z, and 0–9), hyphens (-),
  /// and periods (.).
  pub identifier: String,
  /// The application's publisher. Defaults to the second element in the identifier string.
  /// Currently maps to the Manufacturer property of the Windows Installer.
  pub publisher: Option<String>,
  /// The app's icons
  #[serde(default)]
  pub icon: Vec<String>,
  /// App resources to bundle.
  /// Each resource is a path to a file or directory.
  /// Glob patterns are supported.
  pub resources: Option<BundleResources>,
  /// A copyright string associated with your application.
  pub copyright: Option<String>,
  /// The application kind.
  ///
  /// Should be one of the following:
  /// Business, DeveloperTool, Education, Entertainment, Finance, Game, ActionGame, AdventureGame, ArcadeGame, BoardGame, CardGame, CasinoGame, DiceGame, EducationalGame, FamilyGame, KidsGame, MusicGame, PuzzleGame, RacingGame, RolePlayingGame, SimulationGame, SportsGame, StrategyGame, TriviaGame, WordGame, GraphicsAndDesign, HealthcareAndFitness, Lifestyle, Medical, Music, News, Photography, Productivity, Reference, SocialNetworking, Sports, Travel, Utility, Video, Weather.
  pub category: Option<String>,
  /// File associations to application.
  pub file_associations: Option<Vec<FileAssociation>>,
  /// A short description of your application.
  #[serde(alias = "short-description")]
  pub short_description: Option<String>,
  /// A longer, multi-line description of the application.
  #[serde(alias = "long-description")]
  pub long_description: Option<String>,
  /// Configuration for the AppImage bundle.
  #[serde(default)]
  pub appimage: AppImageConfig,
  /// Configuration for the Debian bundle.
  #[serde(default)]
  pub deb: DebConfig,
  /// Configuration for the RPM bundle.
  #[serde(default)]
  pub rpm: RpmConfig,
  /// DMG-specific settings.
  #[serde(default)]
  pub dmg: DmgConfig,
  /// Configuration for the macOS bundles.
  #[serde(rename = "macOS", default)]
  pub macos: MacConfig,
  /// A list of—either absolute or relative—paths to binaries to embed with your application.
  ///
  /// Note that Tauri will look for system-specific binaries following the pattern "binary-name{-target-triple}{.system-extension}".
  ///
  /// E.g. for the external binary "my-binary", Tauri looks for:
  ///
  /// - "my-binary-x86_64-pc-windows-msvc.exe" for Windows
  /// - "my-binary-x86_64-apple-darwin" for macOS
  /// - "my-binary-x86_64-unknown-linux-gnu" for Linux
  ///
  /// so don't forget to provide binaries for all targeted platforms.
  #[serde(alias = "external-bin")]
  pub external_bin: Option<Vec<String>>,
  /// Configuration for the Windows bundle.
  #[serde(default)]
  pub windows: WindowsConfig,
  /// iOS configuration.
  #[serde(rename = "iOS", default)]
  pub ios: IosConfig,
  /// Android configuration.
  #[serde(default)]
  pub android: AndroidConfig,
  /// The updater configuration.
  #[serde(default)]
  pub updater: UpdaterConfig,
}

/// a tuple struct of RGBA colors. Each value has minimum of 0 and maximum of 255.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl From<Color> for (u8, u8, u8, u8) {
  fn from(value: Color) -> Self {
    (value.0, value.1, value.2, value.3)
  }
}

/// The window effects configuration object
#[skip_serializing_none]
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowEffectsConfig {
  /// List of Window effects to apply to the Window.
  /// Conflicting effects will apply the first one and ignore the rest.
  pub effects: Vec<WindowEffect>,
  /// Window effect state **macOS Only**
  pub state: Option<WindowEffectState>,
  /// Window effect corner radius **macOS Only**
  pub radius: Option<f64>,
  /// Window effect color. Affects [`WindowEffect::Blur`] and [`WindowEffect::Acrylic`] only
  /// on Windows 10 v1903+. Doesn't have any effect on Windows 7 or Windows 11.
  pub color: Option<Color>,
}

/// The window configuration object.
///
/// See more: <https://tauri.app/v1/api/config#windowconfig>
#[skip_serializing_none]
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowConfig {
  /// The window identifier. It must be alphanumeric.
  #[serde(default = "default_window_label")]
  pub label: String,
  /// The window webview URL.
  #[serde(default)]
  pub url: WebviewUrl,
  /// The user agent for the webview
  #[serde(alias = "user-agent")]
  pub user_agent: Option<String>,
  /// Whether the file drop is enabled or not on the webview. By default it is enabled.
  ///
  /// Disabling it is required to use drag and drop on the frontend on Windows.
  #[serde(default = "default_true", alias = "file-drop-enabled")]
  pub file_drop_enabled: bool,
  /// Whether or not the window starts centered or not.
  #[serde(default)]
  pub center: bool,
  /// The horizontal position of the window's top left corner
  pub x: Option<f64>,
  /// The vertical position of the window's top left corner
  pub y: Option<f64>,
  /// The window width.
  #[serde(default = "default_width")]
  pub width: f64,
  /// The window height.
  #[serde(default = "default_height")]
  pub height: f64,
  /// The min window width.
  #[serde(alias = "min-width")]
  pub min_width: Option<f64>,
  /// The min window height.
  #[serde(alias = "min-height")]
  pub min_height: Option<f64>,
  /// The max window width.
  #[serde(alias = "max-width")]
  pub max_width: Option<f64>,
  /// The max window height.
  #[serde(alias = "max-height")]
  pub max_height: Option<f64>,
  /// Whether the window is resizable or not. When resizable is set to false, native window's maximize button is automatically disabled.
  #[serde(default = "default_true")]
  pub resizable: bool,
  /// Whether the window's native maximize button is enabled or not.
  /// If resizable is set to false, this setting is ignored.
  ///
  /// ## Platform-specific
  ///
  /// - **macOS:** Disables the "zoom" button in the window titlebar, which is also used to enter fullscreen mode.
  /// - **Linux / iOS / Android:** Unsupported.
  #[serde(default = "default_true")]
  pub maximizable: bool,
  /// Whether the window's native minimize button is enabled or not.
  ///
  /// ## Platform-specific
  ///
  /// - **Linux / iOS / Android:** Unsupported.
  #[serde(default = "default_true")]
  pub minimizable: bool,
  /// Whether the window's native close button is enabled or not.
  ///
  /// ## Platform-specific
  ///
  /// - **Linux:** "GTK+ will do its best to convince the window manager not to show a close button.
  ///   Depending on the system, this function may not have any effect when called on a window that is already visible"
  /// - **iOS / Android:** Unsupported.
  #[serde(default = "default_true")]
  pub closable: bool,
  /// The window title.
  #[serde(default = "default_title")]
  pub title: String,
  /// Whether the window starts as fullscreen or not.
  #[serde(default)]
  pub fullscreen: bool,
  /// Whether the window will be initially focused or not.
  #[serde(default = "default_true")]
  pub focus: bool,
  /// Whether the window is transparent or not.
  ///
  /// Note that on `macOS` this requires the `macos-private-api` feature flag, enabled under `tauri > macOSPrivateApi`.
  /// WARNING: Using private APIs on `macOS` prevents your application from being accepted to the `App Store`.
  #[serde(default)]
  pub transparent: bool,
  /// Whether the window is maximized or not.
  #[serde(default)]
  pub maximized: bool,
  /// Whether the window is visible or not.
  #[serde(default = "default_true")]
  pub visible: bool,
  /// Whether the window should have borders and bars.
  #[serde(default = "default_true")]
  pub decorations: bool,
  /// Whether the window should always be below other windows.
  #[serde(default, alias = "always-on-bottom")]
  pub always_on_bottom: bool,
  /// Whether the window should always be on top of other windows.
  #[serde(default, alias = "always-on-top")]
  pub always_on_top: bool,
  /// Whether the window should be visible on all workspaces or virtual desktops.
  #[serde(default, alias = "all-workspaces")]
  pub visible_on_all_workspaces: bool,
  /// Prevents the window contents from being captured by other apps.
  #[serde(default, alias = "content-protected")]
  pub content_protected: bool,
  /// If `true`, hides the window icon from the taskbar on Windows and Linux.
  #[serde(default, alias = "skip-taskbar")]
  pub skip_taskbar: bool,
  /// The initial window theme. Defaults to the system theme. Only implemented on Windows and macOS 10.14+.
  pub theme: Option<crate::Theme>,
  /// The style of the macOS title bar.
  #[serde(default, alias = "title-bar-style")]
  pub title_bar_style: TitleBarStyle,
  /// If `true`, sets the window title to be hidden on macOS.
  #[serde(default, alias = "hidden-title")]
  pub hidden_title: bool,
  /// Whether clicking an inactive window also clicks through to the webview on macOS.
  #[serde(default, alias = "accept-first-mouse")]
  pub accept_first_mouse: bool,
  /// Defines the window [tabbing identifier] for macOS.
  ///
  /// Windows with matching tabbing identifiers will be grouped together.
  /// If the tabbing identifier is not set, automatic tabbing will be disabled.
  ///
  /// [tabbing identifier]: <https://developer.apple.com/documentation/appkit/nswindow/1644704-tabbingidentifier>
  #[serde(default, alias = "tabbing-identifier")]
  pub tabbing_identifier: Option<String>,
  /// Defines additional browser arguments on Windows. By default wry passes `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`
  /// so if you use this method, you also need to disable these components by yourself if you want.
  #[serde(default, alias = "additional-browser-args")]
  pub additional_browser_args: Option<String>,
  /// Whether or not the window has shadow.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows:**
  ///   - `false` has no effect on decorated window, shadow are always ON.
  ///   - `true` will make ndecorated window have a 1px white border,
  /// and on Windows 11, it will have a rounded corners.
  /// - **Linux:** Unsupported.
  #[serde(default = "default_true")]
  pub shadow: bool,
  /// Window effects.
  ///
  /// Requires the window to be transparent.
  ///
  /// ## Platform-specific:
  ///
  /// - **Windows**: If using decorations or shadows, you may want to try this workaround <https://github.com/tauri-apps/tao/issues/72#issuecomment-975607891>
  /// - **Linux**: Unsupported
  #[serde(default, alias = "window-effects")]
  pub window_effects: Option<WindowEffectsConfig>,
  /// Whether or not the webview should be launched in incognito  mode.
  ///
  ///  ## Platform-specific:
  ///
  ///  - **Android**: Unsupported.
  #[serde(default)]
  pub incognito: bool,
}

impl Default for WindowConfig {
  fn default() -> Self {
    Self {
      label: default_window_label(),
      url: WebviewUrl::default(),
      user_agent: None,
      file_drop_enabled: true,
      center: false,
      x: None,
      y: None,
      width: default_width(),
      height: default_height(),
      min_width: None,
      min_height: None,
      max_width: None,
      max_height: None,
      resizable: true,
      maximizable: true,
      minimizable: true,
      closable: true,
      title: default_title(),
      fullscreen: false,
      focus: false,
      transparent: false,
      maximized: false,
      visible: true,
      decorations: true,
      always_on_bottom: false,
      always_on_top: false,
      visible_on_all_workspaces: false,
      content_protected: false,
      skip_taskbar: false,
      theme: None,
      title_bar_style: Default::default(),
      hidden_title: false,
      accept_first_mouse: false,
      tabbing_identifier: None,
      additional_browser_args: None,
      shadow: true,
      window_effects: None,
      incognito: false,
    }
  }
}

fn default_window_label() -> String {
  "main".to_string()
}

fn default_width() -> f64 {
  800f64
}

fn default_height() -> f64 {
  600f64
}

fn default_title() -> String {
  "Tauri App".to_string()
}

/// A Content-Security-Policy directive source list.
/// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Security-Policy/Sources#sources>.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", untagged)]
pub enum CspDirectiveSources {
  /// An inline list of CSP sources. Same as [`Self::List`], but concatenated with a space separator.
  Inline(String),
  /// A list of CSP sources. The collection will be concatenated with a space separator for the CSP string.
  List(Vec<String>),
}

impl Default for CspDirectiveSources {
  fn default() -> Self {
    Self::List(Vec::new())
  }
}

impl From<CspDirectiveSources> for Vec<String> {
  fn from(sources: CspDirectiveSources) -> Self {
    match sources {
      CspDirectiveSources::Inline(source) => source.split(' ').map(|s| s.to_string()).collect(),
      CspDirectiveSources::List(l) => l,
    }
  }
}

impl CspDirectiveSources {
  /// Whether the given source is configured on this directive or not.
  pub fn contains(&self, source: &str) -> bool {
    match self {
      Self::Inline(s) => s.contains(&format!("{source} ")) || s.contains(&format!(" {source}")),
      Self::List(l) => l.contains(&source.into()),
    }
  }

  /// Appends the given source to this directive.
  pub fn push<S: AsRef<str>>(&mut self, source: S) {
    match self {
      Self::Inline(s) => {
        s.push(' ');
        s.push_str(source.as_ref());
      }
      Self::List(l) => {
        l.push(source.as_ref().to_string());
      }
    }
  }

  /// Extends this CSP directive source list with the given array of sources.
  pub fn extend(&mut self, sources: Vec<String>) {
    for s in sources {
      self.push(s);
    }
  }
}

/// A Content-Security-Policy definition.
/// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP>.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", untagged)]
pub enum Csp {
  /// The entire CSP policy in a single text string.
  Policy(String),
  /// An object mapping a directive with its sources values as a list of strings.
  DirectiveMap(HashMap<String, CspDirectiveSources>),
}

impl From<HashMap<String, CspDirectiveSources>> for Csp {
  fn from(map: HashMap<String, CspDirectiveSources>) -> Self {
    Self::DirectiveMap(map)
  }
}

impl From<Csp> for HashMap<String, CspDirectiveSources> {
  fn from(csp: Csp) -> Self {
    match csp {
      Csp::Policy(policy) => {
        let mut map = HashMap::new();
        for directive in policy.split(';') {
          let mut tokens = directive.trim().split(' ');
          if let Some(directive) = tokens.next() {
            let sources = tokens.map(|s| s.to_string()).collect::<Vec<String>>();
            map.insert(directive.to_string(), CspDirectiveSources::List(sources));
          }
        }
        map
      }
      Csp::DirectiveMap(m) => m,
    }
  }
}

impl Display for Csp {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Policy(s) => write!(f, "{s}"),
      Self::DirectiveMap(m) => {
        let len = m.len();
        let mut i = 0;
        for (directive, sources) in m {
          let sources: Vec<String> = sources.clone().into();
          write!(f, "{} {}", directive, sources.join(" "))?;
          i += 1;
          if i != len {
            write!(f, "; ")?;
          }
        }
        Ok(())
      }
    }
  }
}

/// The possible values for the `dangerous_disable_asset_csp_modification` config option.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum DisabledCspModificationKind {
  /// If `true`, disables all CSP modification.
  /// `false` is the default value and it configures Tauri to control the CSP.
  Flag(bool),
  /// Disables the given list of CSP directives modifications.
  List(Vec<String>),
}

impl DisabledCspModificationKind {
  /// Determines whether the given CSP directive can be modified or not.
  pub fn can_modify(&self, directive: &str) -> bool {
    match self {
      Self::Flag(f) => !f,
      Self::List(l) => !l.contains(&directive.into()),
    }
  }
}

impl Default for DisabledCspModificationKind {
  fn default() -> Self {
    Self::Flag(false)
  }
}

/// External command access definition.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteDomainAccessScope {
  /// The URL scheme to allow. By default, all schemas are allowed.
  pub scheme: Option<String>,
  /// The domain to allow.
  pub domain: String,
  /// The list of window labels this scope applies to.
  pub windows: Vec<String>,
  /// The list of plugins that are allowed in this scope.
  /// The names should be without the `tauri-plugin-` prefix, for example `"store"` for `tauri-plugin-store`.
  #[serde(default)]
  pub plugins: Vec<String>,
}

/// Protocol scope definition.
/// It is a list of glob patterns that restrict the API access from the webview.
///
/// Each pattern can start with a variable that resolves to a system base directory.
/// The variables are: `$AUDIO`, `$CACHE`, `$CONFIG`, `$DATA`, `$LOCALDATA`, `$DESKTOP`,
/// `$DOCUMENT`, `$DOWNLOAD`, `$EXE`, `$FONT`, `$HOME`, `$PICTURE`, `$PUBLIC`, `$RUNTIME`,
/// `$TEMPLATE`, `$VIDEO`, `$RESOURCE`, `$APP`, `$LOG`, `$TEMP`, `$APPCONFIG`, `$APPDATA`,
/// `$APPLOCALDATA`, `$APPCACHE`, `$APPLOG`.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum FsScope {
  /// A list of paths that are allowed by this scope.
  AllowedPaths(Vec<PathBuf>),
  /// A complete scope configuration.
  #[serde(rename_all = "camelCase")]
  Scope {
    /// A list of paths that are allowed by this scope.
    #[serde(default)]
    allow: Vec<PathBuf>,
    /// A list of paths that are not allowed by this scope.
    /// This gets precedence over the [`Self::Scope::allow`] list.
    #[serde(default)]
    deny: Vec<PathBuf>,
    /// Whether or not paths that contain components that start with a `.`
    /// will require that `.` appears literally in the pattern; `*`, `?`, `**`,
    /// or `[...]` will not match. This is useful because such files are
    /// conventionally considered hidden on Unix systems and it might be
    /// desirable to skip them when listing files.
    ///
    /// Defaults to `true` on Unix systems and `false` on Windows
    // dotfiles are not supposed to be exposed by default on unix
    #[serde(alias = "require-literal-leading-dot")]
    require_literal_leading_dot: Option<bool>,
  },
}

impl Default for FsScope {
  fn default() -> Self {
    Self::AllowedPaths(Vec::new())
  }
}

impl FsScope {
  /// The list of allowed paths.
  pub fn allowed_paths(&self) -> &Vec<PathBuf> {
    match self {
      Self::AllowedPaths(p) => p,
      Self::Scope { allow, .. } => allow,
    }
  }

  /// The list of forbidden paths.
  pub fn forbidden_paths(&self) -> Option<&Vec<PathBuf>> {
    match self {
      Self::AllowedPaths(_) => None,
      Self::Scope { deny, .. } => Some(deny),
    }
  }
}

/// Config for the asset custom protocol.
///
/// See more: <https://tauri.app/v1/api/config#assetprotocolconfig>
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetProtocolConfig {
  /// The access scope for the asset protocol.
  #[serde(default)]
  pub scope: FsScope,
  /// Enables the asset protocol.
  #[serde(default)]
  pub enable: bool,
}

/// Security configuration.
///
/// See more: <https://tauri.app/v1/api/config#securityconfig>
#[skip_serializing_none]
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecurityConfig {
  /// The Content Security Policy that will be injected on all HTML files on the built application.
  /// If [`dev_csp`](#SecurityConfig.devCsp) is not specified, this value is also injected on dev.
  ///
  /// This is a really important part of the configuration since it helps you ensure your WebView is secured.
  /// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP>.
  pub csp: Option<Csp>,
  /// The Content Security Policy that will be injected on all HTML files on development.
  ///
  /// This is a really important part of the configuration since it helps you ensure your WebView is secured.
  /// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP>.
  #[serde(alias = "dev-csp")]
  pub dev_csp: Option<Csp>,
  /// Freeze the `Object.prototype` when using the custom protocol.
  #[serde(default, alias = "freeze-prototype")]
  pub freeze_prototype: bool,
  /// Disables the Tauri-injected CSP sources.
  ///
  /// At compile time, Tauri parses all the frontend assets and changes the Content-Security-Policy
  /// to only allow loading of your own scripts and styles by injecting nonce and hash sources.
  /// This stricts your CSP, which may introduce issues when using along with other flexing sources.
  ///
  /// This configuration option allows both a boolean and a list of strings as value.
  /// A boolean instructs Tauri to disable the injection for all CSP injections,
  /// and a list of strings indicates the CSP directives that Tauri cannot inject.
  ///
  /// **WARNING:** Only disable this if you know what you are doing and have properly configured the CSP.
  /// Your application might be vulnerable to XSS attacks without this Tauri protection.
  #[serde(default, alias = "dangerous-disable-asset-csp-modification")]
  pub dangerous_disable_asset_csp_modification: DisabledCspModificationKind,
  /// Custom protocol config.
  #[serde(default, alias = "asset-protocol")]
  pub asset_protocol: AssetProtocolConfig,
}

/// The application pattern.
#[skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "use", content = "options")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum PatternKind {
  /// Brownfield pattern.
  Brownfield,
  /// Isolation pattern. Recommended for security purposes.
  Isolation {
    /// The dir containing the index.html file that contains the secure isolation application.
    dir: PathBuf,
  },
}

impl Default for PatternKind {
  fn default() -> Self {
    Self::Brownfield
  }
}

/// The Tauri configuration object.
///
/// See more: <https://tauri.app/v1/api/config#tauriconfig>
#[skip_serializing_none]
#[derive(Debug, Default, PartialEq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TauriConfig {
  /// The pattern to use.
  #[serde(default)]
  pub pattern: PatternKind,
  /// The windows configuration.
  #[serde(default)]
  pub windows: Vec<WindowConfig>,
  /// The bundler configuration.
  #[serde(default)]
  pub bundle: BundleConfig,
  /// Security configuration.
  #[serde(default)]
  pub security: SecurityConfig,
  /// Configuration for app tray icon.
  #[serde(alias = "tray-icon")]
  pub tray_icon: Option<TrayIconConfig>,
  /// MacOS private API configuration. Enables the transparent background API and sets the `fullScreenEnabled` preference to `true`.
  #[serde(rename = "macOSPrivateApi", alias = "macos-private-api", default)]
  pub macos_private_api: bool,
}

impl TauriConfig {
  /// Returns all Cargo features.
  pub fn all_features() -> Vec<&'static str> {
    vec![
      "tray-icon",
      "macos-private-api",
      "isolation",
      "protocol-asset",
    ]
  }

  /// Returns the enabled Cargo features.
  pub fn features(&self) -> Vec<&str> {
    let mut features = Vec::new();
    if self.tray_icon.is_some() {
      features.push("tray-icon");
    }
    if self.macos_private_api {
      features.push("macos-private-api");
    }
    if let PatternKind::Isolation { .. } = self.pattern {
      features.push("isolation");
    }
    if self.security.asset_protocol.enable {
      features.push("protocol-asset");
    }
    features.sort_unstable();
    features
  }
}

/// Install modes for the Windows update.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename_all = "camelCase"))]
pub enum WindowsUpdateInstallMode {
  /// Specifies there's a basic UI during the installation process, including a final dialog box at the end.
  BasicUi,
  /// The quiet mode means there's no user interaction required.
  /// Requires admin privileges if the installer does.
  Quiet,
  /// Specifies unattended mode, which means the installation only shows a progress bar.
  Passive,
  // to add more modes, we need to check if the updater relaunch makes sense
  // i.e. for a full UI mode, the user can also mark the installer to start the app
}

impl WindowsUpdateInstallMode {
  /// Returns the associated `msiexec.exe` arguments.
  pub fn msiexec_args(&self) -> &'static [&'static str] {
    match self {
      Self::BasicUi => &["/qb+"],
      Self::Quiet => &["/quiet"],
      Self::Passive => &["/passive"],
    }
  }

  /// Returns the associated nsis arguments.
  pub fn nsis_args(&self) -> &'static [&'static str] {
    match self {
      Self::Passive => &["/P", "/R"],
      Self::Quiet => &["/S", "/R"],
      _ => &[],
    }
  }
}

impl Display for WindowsUpdateInstallMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}",
      match self {
        Self::BasicUi => "basicUI",
        Self::Quiet => "quiet",
        Self::Passive => "passive",
      }
    )
  }
}

impl Default for WindowsUpdateInstallMode {
  fn default() -> Self {
    Self::Passive
  }
}

impl Serialize for WindowsUpdateInstallMode {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(self.to_string().as_ref())
  }
}

impl<'de> Deserialize<'de> for WindowsUpdateInstallMode {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    match s.to_lowercase().as_str() {
      "basicui" => Ok(Self::BasicUi),
      "quiet" => Ok(Self::Quiet),
      "passive" => Ok(Self::Passive),
      _ => Err(DeError::custom(format!(
        "unknown update install mode '{s}'"
      ))),
    }
  }
}

/// The updater configuration for Windows.
///
/// See more: <https://tauri.app/v1/api/config#updaterwindowsconfig>
#[skip_serializing_none]
#[derive(Debug, Default, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdaterWindowsConfig {
  /// The installation mode for the update on Windows. Defaults to `passive`.
  #[serde(default, alias = "install-mode")]
  pub install_mode: WindowsUpdateInstallMode,
}

/// Configuration for application tray icon.
///
/// See more: <https://tauri.app/v1/api/config#trayiconconfig>
#[skip_serializing_none]
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrayIconConfig {
  /// Set an id for this tray icon so you can reference it later, defaults to `main`.
  pub id: Option<String>,
  /// Path to the default icon to use for the tray icon.
  #[serde(alias = "icon-path")]
  pub icon_path: PathBuf,
  /// A Boolean value that determines whether the image represents a [template](https://developer.apple.com/documentation/appkit/nsimage/1520017-template?language=objc) image on macOS.
  #[serde(default, alias = "icon-as-template")]
  pub icon_as_template: bool,
  /// A Boolean value that determines whether the menu should appear when the tray icon receives a left click on macOS.
  #[serde(default = "default_true", alias = "menu-on-left-click")]
  pub menu_on_left_click: bool,
  /// Title for MacOS tray
  pub title: Option<String>,
  /// Tray icon tooltip on Windows and macOS
  pub tooltip: Option<String>,
}

/// General configuration for the iOS target.
#[skip_serializing_none]
#[derive(Debug, Default, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IosConfig {
  /// The development team. This value is required for iOS development because code signing is enforced.
  /// The `APPLE_DEVELOPMENT_TEAM` environment variable can be set to overwrite it.
  #[serde(alias = "development-team")]
  pub development_team: Option<String>,
}

/// General configuration for the iOS target.
#[skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AndroidConfig {
  /// The minimum API level required for the application to run.
  /// The Android system will prevent the user from installing the application if the system's API level is lower than the value specified.
  #[serde(alias = "min-sdk-version", default = "default_min_sdk_version")]
  pub min_sdk_version: u32,
}

impl Default for AndroidConfig {
  fn default() -> Self {
    Self {
      min_sdk_version: default_min_sdk_version(),
    }
  }
}

fn default_min_sdk_version() -> u32 {
  24
}

/// Defines the URL or assets to embed in the application.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged, deny_unknown_fields)]
#[non_exhaustive]
pub enum AppUrl {
  /// The app's external URL, or the path to the directory containing the app assets.
  Url(WebviewUrl),
  /// An array of files to embed on the app.
  Files(Vec<PathBuf>),
}

impl std::fmt::Display for AppUrl {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Url(url) => write!(f, "{url}"),
      Self::Files(files) => write!(f, "{}", serde_json::to_string(files).unwrap()),
    }
  }
}

/// Describes the shell command to run before `tauri dev`.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", untagged)]
pub enum BeforeDevCommand {
  /// Run the given script with the default options.
  Script(String),
  /// Run the given script with custom options.
  ScriptWithOptions {
    /// The script to execute.
    script: String,
    /// The current working directory.
    cwd: Option<String>,
    /// Whether `tauri dev` should wait for the command to finish or not. Defaults to `false`.
    #[serde(default)]
    wait: bool,
  },
}

/// Describes a shell command to be executed when a CLI hook is triggered.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", untagged)]
pub enum HookCommand {
  /// Run the given script with the default options.
  Script(String),
  /// Run the given script with custom options.
  ScriptWithOptions {
    /// The script to execute.
    script: String,
    /// The current working directory.
    cwd: Option<String>,
  },
}

/// The Build configuration object.
///
/// See more: <https://tauri.app/v1/api/config#buildconfig>
#[skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildConfig {
  /// The binary used to build and run the application.
  pub runner: Option<String>,
  /// The path to the application assets or URL to load in development.
  ///
  /// This is usually an URL to a dev server, which serves your application assets
  /// with live reloading. Most modern JavaScript bundlers provides a way to start a dev server by default.
  ///
  /// See [vite](https://vitejs.dev/guide/), [Webpack DevServer](https://webpack.js.org/configuration/dev-server/) and [sirv](https://github.com/lukeed/sirv)
  /// for examples on how to set up a dev server.
  #[serde(default = "default_dev_path", alias = "dev-path")]
  pub dev_path: AppUrl,
  /// The path to the application assets or URL to load in production.
  ///
  /// When a path relative to the configuration file is provided,
  /// it is read recursively and all files are embedded in the application binary.
  /// Tauri then looks for an `index.html` file unless you provide a custom window URL.
  ///
  /// You can also provide a list of paths to be embedded, which allows granular control over what files are added to the binary.
  /// In this case, all files are added to the root and you must reference it that way in your HTML files.
  ///
  /// When an URL is provided, the application won't have bundled assets
  /// and the application will load that URL by default.
  #[serde(default = "default_dist_dir", alias = "dist-dir")]
  pub dist_dir: AppUrl,
  /// A shell command to run before `tauri dev` kicks in.
  ///
  /// The TAURI_ENV_PLATFORM, TAURI_ENV_ARCH, TAURI_ENV_FAMILY, TAURI_ENV_PLATFORM_VERSION, TAURI_ENV_PLATFORM_TYPE and TAURI_ENV_DEBUG environment variables are set if you perform conditional compilation.
  #[serde(alias = "before-dev-command")]
  pub before_dev_command: Option<BeforeDevCommand>,
  /// A shell command to run before `tauri build` kicks in.
  ///
  /// The TAURI_ENV_PLATFORM, TAURI_ENV_ARCH, TAURI_ENV_FAMILY, TAURI_ENV_PLATFORM_VERSION, TAURI_ENV_PLATFORM_TYPE and TAURI_ENV_DEBUG environment variables are set if you perform conditional compilation.
  #[serde(alias = "before-build-command")]
  pub before_build_command: Option<HookCommand>,
  /// A shell command to run before the bundling phase in `tauri build` kicks in.
  ///
  /// The TAURI_ENV_PLATFORM, TAURI_ENV_ARCH, TAURI_ENV_FAMILY, TAURI_ENV_PLATFORM_VERSION, TAURI_ENV_PLATFORM_TYPE and TAURI_ENV_DEBUG environment variables are set if you perform conditional compilation.
  #[serde(alias = "before-bundle-command")]
  pub before_bundle_command: Option<HookCommand>,
  /// Features passed to `cargo` commands.
  pub features: Option<Vec<String>>,
  /// Whether we should inject the Tauri API on `window.__TAURI__` or not.
  #[serde(default, alias = "with-global-tauri")]
  pub with_global_tauri: bool,
}

impl Default for BuildConfig {
  fn default() -> Self {
    Self {
      runner: None,
      dev_path: default_dev_path(),
      dist_dir: default_dist_dir(),
      before_dev_command: None,
      before_build_command: None,
      before_bundle_command: None,
      features: None,
      with_global_tauri: false,
    }
  }
}

fn default_dev_path() -> AppUrl {
  AppUrl::Url(WebviewUrl::External(
    Url::parse("http://localhost:8080").unwrap(),
  ))
}

fn default_dist_dir() -> AppUrl {
  AppUrl::Url(WebviewUrl::App("../dist".into()))
}

#[derive(Debug, PartialEq, Eq)]
struct PackageVersion(String);

impl<'d> serde::Deserialize<'d> for PackageVersion {
  fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
    struct PackageVersionVisitor;

    impl<'d> Visitor<'d> for PackageVersionVisitor {
      type Value = PackageVersion;

      fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
          formatter,
          "a semver string or a path to a package.json file"
        )
      }

      fn visit_str<E: DeError>(self, value: &str) -> Result<PackageVersion, E> {
        let path = PathBuf::from(value);
        if path.exists() {
          let json_str = read_to_string(&path)
            .map_err(|e| DeError::custom(format!("failed to read version JSON file: {e}")))?;
          let package_json: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| DeError::custom(format!("failed to read version JSON file: {e}")))?;
          if let Some(obj) = package_json.as_object() {
            let version = obj
              .get("version")
              .ok_or_else(|| DeError::custom("JSON must contain a `version` field"))?
              .as_str()
              .ok_or_else(|| {
                DeError::custom(format!("`{} > version` must be a string", path.display()))
              })?;
            Ok(PackageVersion(
              Version::from_str(version)
                .map_err(|_| DeError::custom("`package > version` must be a semver string"))?
                .to_string(),
            ))
          } else {
            Err(DeError::custom(
              "`package > version` value is not a path to a JSON object",
            ))
          }
        } else {
          Ok(PackageVersion(
            Version::from_str(value)
              .map_err(|_| DeError::custom("`package > version` must be a semver string"))?
              .to_string(),
          ))
        }
      }
    }

    deserializer.deserialize_string(PackageVersionVisitor {})
  }
}

/// The package configuration.
///
/// See more: <https://tauri.app/v1/api/config#packageconfig>
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageConfig {
  /// App name.
  #[serde(alias = "product-name")]
  #[cfg_attr(feature = "schema", validate(regex(pattern = "^[^/\\:*?\"<>|]+$")))]
  pub product_name: Option<String>,
  /// App version. It is a semver version number or a path to a `package.json` file containing the `version` field. If removed the version number from `Cargo.toml` is used.
  #[serde(deserialize_with = "version_deserializer", default)]
  pub version: Option<String>,
}

fn version_deserializer<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
  D: Deserializer<'de>,
{
  Option::<PackageVersion>::deserialize(deserializer).map(|v| v.map(|v| v.0))
}

impl PackageConfig {
  /// The binary name.
  #[allow(dead_code)]
  pub fn binary_name(&self) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
      self.product_name.as_ref().map(|n| n.to_kebab_case())
    }
    #[cfg(not(target_os = "linux"))]
    {
      self.product_name.clone()
    }
  }
}

/// The Tauri configuration object.
/// It is read from a file where you can define your frontend assets,
/// configure the bundler and define a tray icon.
///
/// The configuration file is generated by the
/// [`tauri init`](https://tauri.app/v1/api/cli#init) command that lives in
/// your Tauri application source directory (src-tauri).
///
/// Once generated, you may modify it at will to customize your Tauri application.
///
/// ## File Formats
///
/// By default, the configuration is defined as a JSON file named `tauri.conf.json`.
///
/// Tauri also supports JSON5 and TOML files via the `config-json5` and `config-toml` Cargo features, respectively.
/// The JSON5 file name must be either `tauri.conf.json` or `tauri.conf.json5`.
/// The TOML file name is `Tauri.toml`.
///
/// ## Platform-Specific Configuration
///
/// In addition to the default configuration file, Tauri can
/// read a platform-specific configuration from `tauri.linux.conf.json`,
/// `tauri.windows.conf.json`, `tauri.macos.conf.json`, `tauri.android.conf.json` and `tauri.ios.conf.json`
/// (or `Tauri.linux.toml`, `Tauri.windows.toml`, `Tauri.macos.toml`, `Tauri.android.toml` and `Tauri.ios.toml` if the `Tauri.toml` format is used),
/// which gets merged with the main configuration object.
///
/// ## Configuration Structure
///
/// The configuration is composed of the following objects:
///
/// - [`package`](#packageconfig): Package settings
/// - [`tauri`](#tauriconfig): The Tauri config
/// - [`build`](#buildconfig): The build configuration
/// - [`plugins`](#pluginconfig): The plugins config
///
/// ```json title="Example tauri.config.json file"
/// {
///   "build": {
///     "beforeBuildCommand": "",
///     "beforeDevCommand": "",
///     "devPath": "../dist",
///     "distDir": "../dist"
///   },
///   "package": {
///     "productName": "tauri-app",
///     "version": "0.1.0"
///   },
///   "tauri": {
///     "bundle": {},
///     "security": {
///       "csp": null
///     },
///     "windows": [
///       {
///         "fullscreen": false,
///         "height": 600,
///         "resizable": true,
///         "title": "Tauri App",
///         "width": 800
///       }
///     ]
///   }
/// }
/// ```
#[skip_serializing_none]
#[derive(Debug, Default, PartialEq, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
  /// The JSON schema for the Tauri config.
  #[serde(rename = "$schema")]
  pub schema: Option<String>,
  /// Package settings.
  #[serde(default)]
  pub package: PackageConfig,
  /// The Tauri configuration.
  #[serde(default)]
  pub tauri: TauriConfig,
  /// The build configuration.
  #[serde(default = "default_build")]
  pub build: BuildConfig,
  /// The plugins config.
  #[serde(default)]
  pub plugins: PluginConfig,
}

/// The plugin configs holds a HashMap mapping a plugin name to its configuration object.
///
/// See more: <https://tauri.app/v1/api/config#pluginconfig>
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct PluginConfig(pub HashMap<String, JsonValue>);

fn default_build() -> BuildConfig {
  BuildConfig {
    runner: None,
    dev_path: default_dev_path(),
    dist_dir: default_dist_dir(),
    before_dev_command: None,
    before_build_command: None,
    before_bundle_command: None,
    features: None,
    with_global_tauri: false,
  }
}

/// Implement `ToTokens` for all config structs, allowing a literal `Config` to be built.
///
/// This allows for a build script to output the values in a `Config` to a `TokenStream`, which can
/// then be consumed by another crate. Useful for passing a config to both the build script and the
/// application using tauri while only parsing it once (in the build script).
#[cfg(feature = "build")]
mod build {
  use super::*;
  use crate::tokens::*;
  use proc_macro2::TokenStream;
  use quote::{quote, ToTokens, TokenStreamExt};
  use std::convert::identity;

  /// Write a `TokenStream` of the `$struct`'s fields to the `$tokens`.
  ///
  /// All fields must represent a binding of the same name that implements `ToTokens`.
  macro_rules! literal_struct {
    ($tokens:ident, $struct:ident, $($field:ident),+) => {
      $tokens.append_all(quote! {
        ::tauri::utils::config::$struct {
          $($field: #$field),+
        }
      })
    };
  }

  impl ToTokens for WebviewUrl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::WebviewUrl };

      tokens.append_all(match self {
        Self::App(path) => {
          let path = path_buf_lit(path);
          quote! { #prefix::App(#path) }
        }
        Self::External(url) => {
          let url = url_lit(url);
          quote! { #prefix::External(#url) }
        }
      })
    }
  }

  impl ToTokens for crate::Theme {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::Theme };

      tokens.append_all(match self {
        Self::Light => quote! { #prefix::Light },
        Self::Dark => quote! { #prefix::Dark },
      })
    }
  }

  impl ToTokens for Color {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let Color(r, g, b, a) = self;
      tokens.append_all(quote! {::tauri::utils::Color(#r,#g,#b,#a)});
    }
  }
  impl ToTokens for WindowEffectsConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let effects = vec_lit(self.effects.clone(), |d| d);
      let state = opt_lit(self.state.as_ref());
      let radius = opt_lit(self.radius.as_ref());
      let color = opt_lit(self.color.as_ref());

      literal_struct!(tokens, WindowEffectsConfig, effects, state, radius, color)
    }
  }

  impl ToTokens for crate::TitleBarStyle {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::TitleBarStyle };

      tokens.append_all(match self {
        Self::Visible => quote! { #prefix::Visible },
        Self::Transparent => quote! { #prefix::Transparent },
        Self::Overlay => quote! { #prefix::Overlay },
      })
    }
  }

  impl ToTokens for crate::WindowEffect {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::WindowEffect };

      #[allow(deprecated)]
      tokens.append_all(match self {
        WindowEffect::AppearanceBased => quote! { #prefix::AppearanceBased},
        WindowEffect::Light => quote! { #prefix::Light},
        WindowEffect::Dark => quote! { #prefix::Dark},
        WindowEffect::MediumLight => quote! { #prefix::MediumLight},
        WindowEffect::UltraDark => quote! { #prefix::UltraDark},
        WindowEffect::Titlebar => quote! { #prefix::Titlebar},
        WindowEffect::Selection => quote! { #prefix::Selection},
        WindowEffect::Menu => quote! { #prefix::Menu},
        WindowEffect::Popover => quote! { #prefix::Popover},
        WindowEffect::Sidebar => quote! { #prefix::Sidebar},
        WindowEffect::HeaderView => quote! { #prefix::HeaderView},
        WindowEffect::Sheet => quote! { #prefix::Sheet},
        WindowEffect::WindowBackground => quote! { #prefix::WindowBackground},
        WindowEffect::HudWindow => quote! { #prefix::HudWindow},
        WindowEffect::FullScreenUI => quote! { #prefix::FullScreenUI},
        WindowEffect::Tooltip => quote! { #prefix::Tooltip},
        WindowEffect::ContentBackground => quote! { #prefix::ContentBackground},
        WindowEffect::UnderWindowBackground => quote! { #prefix::UnderWindowBackground},
        WindowEffect::UnderPageBackground => quote! { #prefix::UnderPageBackground},
        WindowEffect::Mica => quote! { #prefix::Mica},
        WindowEffect::MicaDark => quote! { #prefix::MicaDark},
        WindowEffect::MicaLight => quote! { #prefix::MicaLight},
        WindowEffect::Blur => quote! { #prefix::Blur},
        WindowEffect::Acrylic => quote! { #prefix::Acrylic},
        WindowEffect::Tabbed => quote! { #prefix::Tabbed },
        WindowEffect::TabbedDark => quote! { #prefix::TabbedDark },
        WindowEffect::TabbedLight => quote! { #prefix::TabbedLight },
      })
    }
  }

  impl ToTokens for crate::WindowEffectState {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::WindowEffectState };

      #[allow(deprecated)]
      tokens.append_all(match self {
        WindowEffectState::Active => quote! { #prefix::Active},
        WindowEffectState::FollowsWindowActiveState => quote! { #prefix::FollowsWindowActiveState},
        WindowEffectState::Inactive => quote! { #prefix::Inactive},
      })
    }
  }

  impl ToTokens for WindowConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let label = str_lit(&self.label);
      let url = &self.url;
      let user_agent = opt_str_lit(self.user_agent.as_ref());
      let file_drop_enabled = self.file_drop_enabled;
      let center = self.center;
      let x = opt_lit(self.x.as_ref());
      let y = opt_lit(self.y.as_ref());
      let width = self.width;
      let height = self.height;
      let min_width = opt_lit(self.min_width.as_ref());
      let min_height = opt_lit(self.min_height.as_ref());
      let max_width = opt_lit(self.max_width.as_ref());
      let max_height = opt_lit(self.max_height.as_ref());
      let resizable = self.resizable;
      let maximizable = self.maximizable;
      let minimizable = self.minimizable;
      let closable = self.closable;
      let title = str_lit(&self.title);
      let fullscreen = self.fullscreen;
      let focus = self.focus;
      let transparent = self.transparent;
      let maximized = self.maximized;
      let visible = self.visible;
      let decorations = self.decorations;
      let always_on_bottom = self.always_on_bottom;
      let always_on_top = self.always_on_top;
      let visible_on_all_workspaces = self.visible_on_all_workspaces;
      let content_protected = self.content_protected;
      let skip_taskbar = self.skip_taskbar;
      let theme = opt_lit(self.theme.as_ref());
      let title_bar_style = &self.title_bar_style;
      let hidden_title = self.hidden_title;
      let accept_first_mouse = self.accept_first_mouse;
      let tabbing_identifier = opt_str_lit(self.tabbing_identifier.as_ref());
      let additional_browser_args = opt_str_lit(self.additional_browser_args.as_ref());
      let shadow = self.shadow;
      let window_effects = opt_lit(self.window_effects.as_ref());
      let incognito = self.incognito;

      literal_struct!(
        tokens,
        WindowConfig,
        label,
        url,
        user_agent,
        file_drop_enabled,
        center,
        x,
        y,
        width,
        height,
        min_width,
        min_height,
        max_width,
        max_height,
        resizable,
        maximizable,
        minimizable,
        closable,
        title,
        fullscreen,
        focus,
        transparent,
        maximized,
        visible,
        decorations,
        always_on_bottom,
        always_on_top,
        visible_on_all_workspaces,
        content_protected,
        skip_taskbar,
        theme,
        title_bar_style,
        hidden_title,
        accept_first_mouse,
        tabbing_identifier,
        additional_browser_args,
        shadow,
        window_effects,
        incognito
      );
    }
  }

  impl ToTokens for PatternKind {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::PatternKind };

      tokens.append_all(match self {
        Self::Brownfield => quote! { #prefix::Brownfield },
        #[cfg(not(feature = "isolation"))]
        Self::Isolation { dir: _ } => quote! { #prefix::Brownfield },
        #[cfg(feature = "isolation")]
        Self::Isolation { dir } => {
          let dir = path_buf_lit(dir);
          quote! { #prefix::Isolation { dir: #dir } }
        }
      })
    }
  }

  impl ToTokens for WebviewInstallMode {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::WebviewInstallMode };

      tokens.append_all(match self {
        Self::Skip => quote! { #prefix::Skip },
        Self::DownloadBootstrapper { silent } => {
          quote! { #prefix::DownloadBootstrapper { silent: #silent } }
        }
        Self::EmbedBootstrapper { silent } => {
          quote! { #prefix::EmbedBootstrapper { silent: #silent } }
        }
        Self::OfflineInstaller { silent } => {
          quote! { #prefix::OfflineInstaller { silent: #silent } }
        }
        Self::FixedRuntime { path } => {
          let path = path_buf_lit(path);
          quote! { #prefix::FixedRuntime { path: #path } }
        }
      })
    }
  }

  impl ToTokens for WindowsConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let webview_install_mode = if let Some(fixed_runtime_path) = &self.webview_fixed_runtime_path
      {
        WebviewInstallMode::FixedRuntime {
          path: fixed_runtime_path.clone(),
        }
      } else {
        self.webview_install_mode.clone()
      };
      tokens.append_all(quote! { ::tauri::utils::config::WindowsConfig {
        webview_install_mode: #webview_install_mode,
        ..Default::default()
      }})
    }
  }

  impl ToTokens for UpdaterConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let active = self.active;
      let pubkey = str_lit(&self.pubkey);
      let windows = &self.windows;

      literal_struct!(tokens, UpdaterConfig, active, pubkey, windows);
    }
  }

  impl ToTokens for BundleConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let identifier = str_lit(&self.identifier);
      let publisher = quote!(None);
      let icon = vec_lit(&self.icon, str_lit);
      let active = self.active;
      let targets = quote!(Default::default());
      let resources = quote!(None);
      let copyright = quote!(None);
      let category = quote!(None);
      let file_associations = quote!(None);
      let short_description = quote!(None);
      let long_description = quote!(None);
      let appimage = quote!(Default::default());
      let deb = quote!(Default::default());
      let rpm = quote!(Default::default());
      let dmg = quote!(Default::default());
      let macos = quote!(Default::default());
      let external_bin = opt_vec_str_lit(self.external_bin.as_ref());
      let windows = &self.windows;
      let ios = quote!(Default::default());
      let android = quote!(Default::default());
      let updater = &self.updater;

      literal_struct!(
        tokens,
        BundleConfig,
        active,
        identifier,
        publisher,
        icon,
        targets,
        resources,
        copyright,
        category,
        file_associations,
        short_description,
        long_description,
        appimage,
        deb,
        rpm,
        dmg,
        macos,
        external_bin,
        windows,
        ios,
        android,
        updater
      );
    }
  }

  impl ToTokens for AppUrl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::AppUrl };

      tokens.append_all(match self {
        Self::Url(url) => {
          quote! { #prefix::Url(#url) }
        }
        Self::Files(files) => {
          let files = vec_lit(files, path_buf_lit);
          quote! { #prefix::Files(#files) }
        }
      })
    }
  }

  impl ToTokens for BuildConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let dev_path = &self.dev_path;
      let dist_dir = &self.dist_dir;
      let with_global_tauri = self.with_global_tauri;
      let runner = quote!(None);
      let before_dev_command = quote!(None);
      let before_build_command = quote!(None);
      let before_bundle_command = quote!(None);
      let features = quote!(None);

      literal_struct!(
        tokens,
        BuildConfig,
        runner,
        dev_path,
        dist_dir,
        with_global_tauri,
        before_dev_command,
        before_build_command,
        before_bundle_command,
        features
      );
    }
  }

  impl ToTokens for WindowsUpdateInstallMode {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::WindowsUpdateInstallMode };

      tokens.append_all(match self {
        Self::BasicUi => quote! { #prefix::BasicUi },
        Self::Quiet => quote! { #prefix::Quiet },
        Self::Passive => quote! { #prefix::Passive },
      })
    }
  }

  impl ToTokens for UpdaterWindowsConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let install_mode = &self.install_mode;
      literal_struct!(tokens, UpdaterWindowsConfig, install_mode);
    }
  }

  impl ToTokens for CspDirectiveSources {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::CspDirectiveSources };

      tokens.append_all(match self {
        Self::Inline(sources) => {
          let sources = sources.as_str();
          quote!(#prefix::Inline(#sources.into()))
        }
        Self::List(list) => {
          let list = vec_lit(list, str_lit);
          quote!(#prefix::List(#list))
        }
      })
    }
  }

  impl ToTokens for Csp {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::Csp };

      tokens.append_all(match self {
        Self::Policy(policy) => {
          let policy = policy.as_str();
          quote!(#prefix::Policy(#policy.into()))
        }
        Self::DirectiveMap(list) => {
          let map = map_lit(
            quote! { ::std::collections::HashMap },
            list,
            str_lit,
            identity,
          );
          quote!(#prefix::DirectiveMap(#map))
        }
      })
    }
  }

  impl ToTokens for DisabledCspModificationKind {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::DisabledCspModificationKind };

      tokens.append_all(match self {
        Self::Flag(flag) => {
          quote! { #prefix::Flag(#flag) }
        }
        Self::List(directives) => {
          let directives = vec_lit(directives, str_lit);
          quote! { #prefix::List(#directives) }
        }
      });
    }
  }

  impl ToTokens for RemoteDomainAccessScope {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let scheme = opt_str_lit(self.scheme.as_ref());
      let domain = str_lit(&self.domain);
      let windows = vec_lit(&self.windows, str_lit);
      let plugins = vec_lit(&self.plugins, str_lit);

      literal_struct!(
        tokens,
        RemoteDomainAccessScope,
        scheme,
        domain,
        windows,
        plugins
      );
    }
  }

  impl ToTokens for SecurityConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let csp = opt_lit(self.csp.as_ref());
      let dev_csp = opt_lit(self.dev_csp.as_ref());
      let freeze_prototype = self.freeze_prototype;
      let dangerous_disable_asset_csp_modification = &self.dangerous_disable_asset_csp_modification;
      let asset_protocol = &self.asset_protocol;

      literal_struct!(
        tokens,
        SecurityConfig,
        csp,
        dev_csp,
        freeze_prototype,
        dangerous_disable_asset_csp_modification,
        asset_protocol
      );
    }
  }

  impl ToTokens for TrayIconConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let id = opt_str_lit(self.id.as_ref());
      let icon_as_template = self.icon_as_template;
      let menu_on_left_click = self.menu_on_left_click;
      let icon_path = path_buf_lit(&self.icon_path);
      let title = opt_str_lit(self.title.as_ref());
      let tooltip = opt_str_lit(self.tooltip.as_ref());
      literal_struct!(
        tokens,
        TrayIconConfig,
        id,
        icon_path,
        icon_as_template,
        menu_on_left_click,
        title,
        tooltip
      );
    }
  }

  impl ToTokens for FsScope {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let prefix = quote! { ::tauri::utils::config::FsScope };

      tokens.append_all(match self {
        Self::AllowedPaths(allow) => {
          let allowed_paths = vec_lit(allow, path_buf_lit);
          quote! { #prefix::AllowedPaths(#allowed_paths) }
        }
        Self::Scope { allow, deny , require_literal_leading_dot} => {
          let allow = vec_lit(allow, path_buf_lit);
          let deny = vec_lit(deny, path_buf_lit);
          let  require_literal_leading_dot = opt_lit(require_literal_leading_dot.as_ref());
          quote! { #prefix::Scope { allow: #allow, deny: #deny, require_literal_leading_dot: #require_literal_leading_dot } }
        }
      });
    }
  }

  impl ToTokens for AssetProtocolConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let scope = &self.scope;
      tokens.append_all(quote! { ::tauri::utils::config::AssetProtocolConfig { scope: #scope, ..Default::default() } })
    }
  }

  impl ToTokens for TauriConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let pattern = &self.pattern;
      let windows = vec_lit(&self.windows, identity);
      let bundle = &self.bundle;
      let security = &self.security;
      let tray_icon = opt_lit(self.tray_icon.as_ref());
      let macos_private_api = self.macos_private_api;

      literal_struct!(
        tokens,
        TauriConfig,
        pattern,
        windows,
        bundle,
        security,
        tray_icon,
        macos_private_api
      );
    }
  }

  impl ToTokens for PluginConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let config = map_lit(
        quote! { ::std::collections::HashMap },
        &self.0,
        str_lit,
        json_value_lit,
      );
      tokens.append_all(quote! { ::tauri::utils::config::PluginConfig(#config) })
    }
  }

  impl ToTokens for PackageConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let product_name = opt_str_lit(self.product_name.as_ref());
      let version = opt_str_lit(self.version.as_ref());

      literal_struct!(tokens, PackageConfig, product_name, version);
    }
  }

  impl ToTokens for Config {
    fn to_tokens(&self, tokens: &mut TokenStream) {
      let schema = quote!(None);
      let package = &self.package;
      let tauri = &self.tauri;
      let build = &self.build;
      let plugins = &self.plugins;

      literal_struct!(tokens, Config, schema, package, tauri, build, plugins);
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  // TODO: create a test that compares a config to a json config

  #[test]
  // test all of the default functions
  fn test_defaults() {
    // get default tauri config
    let t_config = TauriConfig::default();
    // get default build config
    let b_config = BuildConfig::default();
    // get default dev path
    let d_path = default_dev_path();
    // get default window
    let d_windows: Vec<WindowConfig> = vec![];
    // get default bundle
    let d_bundle = BundleConfig::default();

    // create a tauri config.
    let tauri = TauriConfig {
      pattern: Default::default(),
      windows: vec![],
      bundle: BundleConfig {
        active: false,
        targets: Default::default(),
        identifier: String::from(""),
        publisher: None,
        icon: Vec::new(),
        resources: None,
        copyright: None,
        category: None,
        file_associations: None,
        short_description: None,
        long_description: None,
        appimage: Default::default(),
        deb: Default::default(),
        rpm: Default::default(),
        dmg: Default::default(),
        macos: Default::default(),
        external_bin: None,
        windows: Default::default(),
        ios: Default::default(),
        android: Default::default(),
        updater: Default::default(),
      },
      security: SecurityConfig {
        csp: None,
        dev_csp: None,
        freeze_prototype: false,
        dangerous_disable_asset_csp_modification: DisabledCspModificationKind::Flag(false),
        asset_protocol: AssetProtocolConfig::default(),
      },
      tray_icon: None,
      macos_private_api: false,
    };

    // create a build config
    let build = BuildConfig {
      runner: None,
      dev_path: AppUrl::Url(WebviewUrl::External(
        Url::parse("http://localhost:8080").unwrap(),
      )),
      dist_dir: AppUrl::Url(WebviewUrl::App("../dist".into())),
      before_dev_command: None,
      before_build_command: None,
      before_bundle_command: None,
      features: None,
      with_global_tauri: false,
    };

    // test the configs
    assert_eq!(t_config, tauri);
    assert_eq!(b_config, build);
    assert_eq!(d_bundle, tauri.bundle);
    assert_eq!(
      d_path,
      AppUrl::Url(WebviewUrl::External(
        Url::parse("http://localhost:8080").unwrap()
      ))
    );
    assert_eq!(d_windows, tauri.windows);
  }
}
