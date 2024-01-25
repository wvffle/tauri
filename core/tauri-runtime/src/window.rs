// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! A layer between raw [`Runtime`] windows and Tauri.

use crate::{
  webview::{DetachedWebview, PendingWebview},
  Icon, Runtime, UserEvent, WindowDispatch,
};

use serde::{Deserialize, Deserializer};
use tauri_utils::{config::WindowConfig, Theme};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

use std::{
  hash::{Hash, Hasher},
  marker::PhantomData,
  path::PathBuf,
  sync::mpsc::Sender,
};

use self::dpi::PhysicalPosition;

/// UI scaling utilities.
pub mod dpi;

/// An event from a window.
#[derive(Debug, Clone)]
pub enum WindowEvent {
  /// The size of the window has changed. Contains the client area's new dimensions.
  Resized(dpi::PhysicalSize<u32>),
  /// The position of the window has changed. Contains the window's new position.
  Moved(dpi::PhysicalPosition<i32>),
  /// The window has been requested to close.
  CloseRequested {
    /// A signal sender. If a `true` value is emitted, the window won't be closed.
    signal_tx: Sender<bool>,
  },
  /// The window has been destroyed.
  Destroyed,
  /// The window gained or lost focus.
  ///
  /// The parameter is true if the window has gained focus, and false if it has lost focus.
  Focused(bool),
  /// The window's scale factor has changed.
  ///
  /// The following user actions can cause DPI changes:
  ///
  /// - Changing the display's resolution.
  /// - Changing the display's scale factor (e.g. in Control Panel on Windows).
  /// - Moving the window to a display with a different scale factor.
  ScaleFactorChanged {
    /// The new scale factor.
    scale_factor: f64,
    /// The window inner size.
    new_inner_size: dpi::PhysicalSize<u32>,
  },
  /// An event associated with the file drop action.
  FileDrop(FileDropEvent),
  /// The system window theme has changed.
  ///
  /// Applications might wish to react to this to change the theme of the content of the window when the system changes the window theme.
  ThemeChanged(Theme),
}

/// The file drop event payload.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum FileDropEvent {
  /// The file(s) have been dragged onto the window, but have not been dropped yet.
  Hovered {
    paths: Vec<PathBuf>,
    /// The position of the mouse cursor.
    position: PhysicalPosition<f64>,
  },
  /// The file(s) have been dropped onto the window.
  Dropped {
    paths: Vec<PathBuf>,
    /// The position of the mouse cursor.
    position: PhysicalPosition<f64>,
  },
  /// The file drop was aborted.
  Cancelled,
}

/// Describes the appearance of the mouse cursor.
#[non_exhaustive]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CursorIcon {
  /// The platform-dependent default cursor.
  #[default]
  Default,
  /// A simple crosshair.
  Crosshair,
  /// A hand (often used to indicate links in web browsers).
  Hand,
  /// Self explanatory.
  Arrow,
  /// Indicates something is to be moved.
  Move,
  /// Indicates text that may be selected or edited.
  Text,
  /// Program busy indicator.
  Wait,
  /// Help indicator (often rendered as a "?")
  Help,
  /// Progress indicator. Shows that processing is being done. But in contrast
  /// with "Wait" the user may still interact with the program. Often rendered
  /// as a spinning beach ball, or an arrow with a watch or hourglass.
  Progress,

  /// Cursor showing that something cannot be done.
  NotAllowed,
  ContextMenu,
  Cell,
  VerticalText,
  Alias,
  Copy,
  NoDrop,
  /// Indicates something can be grabbed.
  Grab,
  /// Indicates something is grabbed.
  Grabbing,
  AllScroll,
  ZoomIn,
  ZoomOut,

  /// Indicate that some edge is to be moved. For example, the 'SeResize' cursor
  /// is used when the movement starts from the south-east corner of the box.
  EResize,
  NResize,
  NeResize,
  NwResize,
  SResize,
  SeResize,
  SwResize,
  WResize,
  EwResize,
  NsResize,
  NeswResize,
  NwseResize,
  ColResize,
  RowResize,
}

impl<'de> Deserialize<'de> for CursorIcon {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    Ok(match s.to_lowercase().as_str() {
      "default" => CursorIcon::Default,
      "crosshair" => CursorIcon::Crosshair,
      "hand" => CursorIcon::Hand,
      "arrow" => CursorIcon::Arrow,
      "move" => CursorIcon::Move,
      "text" => CursorIcon::Text,
      "wait" => CursorIcon::Wait,
      "help" => CursorIcon::Help,
      "progress" => CursorIcon::Progress,
      "notallowed" => CursorIcon::NotAllowed,
      "contextmenu" => CursorIcon::ContextMenu,
      "cell" => CursorIcon::Cell,
      "verticaltext" => CursorIcon::VerticalText,
      "alias" => CursorIcon::Alias,
      "copy" => CursorIcon::Copy,
      "nodrop" => CursorIcon::NoDrop,
      "grab" => CursorIcon::Grab,
      "grabbing" => CursorIcon::Grabbing,
      "allscroll" => CursorIcon::AllScroll,
      "zoomin" => CursorIcon::ZoomIn,
      "zoomout" => CursorIcon::ZoomOut,
      "eresize" => CursorIcon::EResize,
      "nresize" => CursorIcon::NResize,
      "neresize" => CursorIcon::NeResize,
      "nwresize" => CursorIcon::NwResize,
      "sresize" => CursorIcon::SResize,
      "seresize" => CursorIcon::SeResize,
      "swresize" => CursorIcon::SwResize,
      "wresize" => CursorIcon::WResize,
      "ewresize" => CursorIcon::EwResize,
      "nsresize" => CursorIcon::NsResize,
      "neswresize" => CursorIcon::NeswResize,
      "nwseresize" => CursorIcon::NwseResize,
      "colresize" => CursorIcon::ColResize,
      "rowresize" => CursorIcon::RowResize,
      _ => CursorIcon::Default,
    })
  }
}

/// Do **NOT** implement this trait except for use in a custom [`Runtime`]
///
/// This trait is separate from [`WindowBuilder`] to prevent "accidental" implementation.
pub trait WindowBuilderBase: std::fmt::Debug + Clone + Sized {}

/// A builder for all attributes related to a single window.
///
/// This trait is only meant to be implemented by a custom [`Runtime`]
/// and not by applications.
pub trait WindowBuilder: WindowBuilderBase {
  /// Initializes a new window attributes builder.
  fn new() -> Self;

  /// Initializes a new window builder from a [`WindowConfig`]
  fn with_config(config: WindowConfig) -> Self;

  /// Show window in the center of the screen.
  #[must_use]
  fn center(self) -> Self;

  /// The initial position of the window's.
  #[must_use]
  fn position(self, x: f64, y: f64) -> Self;

  /// Window size.
  #[must_use]
  fn inner_size(self, width: f64, height: f64) -> Self;

  /// Window min inner size.
  #[must_use]
  fn min_inner_size(self, min_width: f64, min_height: f64) -> Self;

  /// Window max inner size.
  #[must_use]
  fn max_inner_size(self, max_width: f64, max_height: f64) -> Self;

  /// Whether the window is resizable or not.
  /// When resizable is set to false, native window's maximize button is automatically disabled.
  #[must_use]
  fn resizable(self, resizable: bool) -> Self;

  /// Whether the window's native maximize button is enabled or not.
  /// If resizable is set to false, this setting is ignored.
  ///
  /// ## Platform-specific
  ///
  /// - **macOS:** Disables the "zoom" button in the window titlebar, which is also used to enter fullscreen mode.
  /// - **Linux / iOS / Android:** Unsupported.
  #[must_use]
  fn maximizable(self, maximizable: bool) -> Self;

  /// Whether the window's native minimize button is enabled or not.
  ///
  /// ## Platform-specific
  ///
  /// - **Linux / iOS / Android:** Unsupported.
  #[must_use]
  fn minimizable(self, minimizable: bool) -> Self;

  /// Whether the window's native close button is enabled or not.
  ///
  /// ## Platform-specific
  ///
  /// - **Linux:** "GTK+ will do its best to convince the window manager not to show a close button.
  ///   Depending on the system, this function may not have any effect when called on a window that is already visible"
  /// - **iOS / Android:** Unsupported.
  #[must_use]
  fn closable(self, closable: bool) -> Self;

  /// The title of the window in the title bar.
  #[must_use]
  fn title<S: Into<String>>(self, title: S) -> Self;

  /// Whether to start the window in fullscreen or not.
  #[must_use]
  fn fullscreen(self, fullscreen: bool) -> Self;

  /// Whether the window will be initially focused or not.
  #[must_use]
  fn focused(self, focused: bool) -> Self;

  /// Whether the window should be maximized upon creation.
  #[must_use]
  fn maximized(self, maximized: bool) -> Self;

  /// Whether the window should be immediately visible upon creation.
  #[must_use]
  fn visible(self, visible: bool) -> Self;

  /// Whether the window should be transparent. If this is true, writing colors
  /// with alpha values different than `1.0` will produce a transparent window.
  #[cfg(any(not(target_os = "macos"), feature = "macos-private-api"))]
  #[cfg_attr(
    docsrs,
    doc(cfg(any(not(target_os = "macos"), feature = "macos-private-api")))
  )]
  #[must_use]
  fn transparent(self, transparent: bool) -> Self;

  /// Whether the window should have borders and bars.
  #[must_use]
  fn decorations(self, decorations: bool) -> Self;

  /// Whether the window should always be below other windows.
  #[must_use]
  fn always_on_bottom(self, always_on_bottom: bool) -> Self;

  /// Whether the window should always be on top of other windows.
  #[must_use]
  fn always_on_top(self, always_on_top: bool) -> Self;

  /// Whether the window should be visible on all workspaces or virtual desktops.
  #[must_use]
  fn visible_on_all_workspaces(self, visible_on_all_workspaces: bool) -> Self;

  /// Prevents the window contents from being captured by other apps.
  #[must_use]
  fn content_protected(self, protected: bool) -> Self;

  /// Sets the window icon.
  fn icon(self, icon: Icon) -> crate::Result<Self>;

  /// Sets whether or not the window icon should be added to the taskbar.
  #[must_use]
  fn skip_taskbar(self, skip: bool) -> Self;

  /// Sets whether or not the window has shadow.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows:**
  ///   - `false` has no effect on decorated window, shadows are always ON.
  ///   - `true` will make ndecorated window have a 1px white border,
  /// and on Windows 11, it will have a rounded corners.
  /// - **Linux:** Unsupported.
  #[must_use]
  fn shadow(self, enable: bool) -> Self;

  /// Sets a parent to the window to be created.
  ///
  /// A child window has the WS_CHILD style and is confined to the client area of its parent window.
  ///
  /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#child-windows>
  #[cfg(windows)]
  #[must_use]
  fn parent_window(self, parent: HWND) -> Self;

  /// Sets a parent to the window to be created.
  ///
  /// A child window has the WS_CHILD style and is confined to the client area of its parent window.
  ///
  /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#child-windows>
  #[cfg(target_os = "macos")]
  #[must_use]
  fn parent_window(self, parent: *mut std::ffi::c_void) -> Self;

  /// Set an owner to the window to be created.
  ///
  /// From MSDN:
  /// - An owned window is always above its owner in the z-order.
  /// - The system automatically destroys an owned window when its owner is destroyed.
  /// - An owned window is hidden when its owner is minimized.
  ///
  /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows>
  #[cfg(windows)]
  #[must_use]
  fn owner_window(self, owner: HWND) -> Self;

  /// Enables or disables drag and drop support.
  #[cfg(windows)]
  #[must_use]
  fn drag_and_drop(self, enabled: bool) -> Self;

  /// Hide the titlebar. Titlebar buttons will still be visible.
  #[cfg(target_os = "macos")]
  #[must_use]
  fn title_bar_style(self, style: tauri_utils::TitleBarStyle) -> Self;

  /// Hide the window title.
  #[cfg(target_os = "macos")]
  #[must_use]
  fn hidden_title(self, hidden: bool) -> Self;

  /// Defines the window [tabbing identifier] for macOS.
  ///
  /// Windows with matching tabbing identifiers will be grouped together.
  /// If the tabbing identifier is not set, automatic tabbing will be disabled.
  ///
  /// [tabbing identifier]: <https://developer.apple.com/documentation/appkit/nswindow/1644704-tabbingidentifier>
  #[cfg(target_os = "macos")]
  #[must_use]
  fn tabbing_identifier(self, identifier: &str) -> Self;

  /// Forces a theme or uses the system settings if None was provided.
  fn theme(self, theme: Option<Theme>) -> Self;

  /// Whether the icon was set or not.
  fn has_icon(&self) -> bool;
}

/// A window that has yet to be built.
pub struct PendingWindow<T: UserEvent, R: Runtime<T>> {
  /// The label that the window will be named.
  pub label: String,

  /// The [`WindowBuilder`] that the window will be created with.
  pub window_builder: <R::WindowDispatcher as WindowDispatch<T>>::WindowBuilder,

  /// The webview that gets added to the window. Optional in case you want to use child webviews or other window content instead.
  pub webview: Option<PendingWebview<T, R>>,
}

pub fn is_label_valid(label: &str) -> bool {
  label
    .chars()
    .all(|c| char::is_alphanumeric(c) || c == '-' || c == '/' || c == ':' || c == '_')
}

pub fn assert_label_is_valid(label: &str) {
  assert!(
    is_label_valid(label),
    "Window label must include only alphanumeric characters, `-`, `/`, `:` and `_`."
  );
}

impl<T: UserEvent, R: Runtime<T>> PendingWindow<T, R> {
  /// Create a new [`PendingWindow`] with a label from the given [`WindowBuilder`].
  pub fn new(
    window_builder: <R::WindowDispatcher as WindowDispatch<T>>::WindowBuilder,
    label: impl Into<String>,
  ) -> crate::Result<Self> {
    let label = label.into();
    if !is_label_valid(&label) {
      Err(crate::Error::InvalidWindowLabel)
    } else {
      Ok(Self {
        window_builder,
        label,
        webview: None,
      })
    }
  }

  /// Sets a webview to be created on the window.
  pub fn set_webview(&mut self, webview: PendingWebview<T, R>) -> &mut Self {
    self.webview.replace(webview);
    self
  }
}

/// Identifier of a window.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct WindowId(u32);

impl From<u32> for WindowId {
  fn from(value: u32) -> Self {
    Self(value)
  }
}

/// A window that is not yet managed by Tauri.
#[derive(Debug)]
pub struct DetachedWindow<T: UserEvent, R: Runtime<T>> {
  /// The identifier of the window.
  pub id: WindowId,
  /// Name of the window
  pub label: String,

  /// The [`WindowDispatch`] associated with the window.
  pub dispatcher: R::WindowDispatcher,

  /// The webview dispatcher in case this window has an attached webview.
  pub webview: Option<DetachedWebview<T, R>>,
}

impl<T: UserEvent, R: Runtime<T>> Clone for DetachedWindow<T, R> {
  fn clone(&self) -> Self {
    Self {
      id: self.id,
      label: self.label.clone(),
      dispatcher: self.dispatcher.clone(),
      webview: self.webview.clone(),
    }
  }
}

impl<T: UserEvent, R: Runtime<T>> Hash for DetachedWindow<T, R> {
  /// Only use the [`DetachedWindow`]'s label to represent its hash.
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.label.hash(state)
  }
}

impl<T: UserEvent, R: Runtime<T>> Eq for DetachedWindow<T, R> {}
impl<T: UserEvent, R: Runtime<T>> PartialEq for DetachedWindow<T, R> {
  /// Only use the [`DetachedWindow`]'s label to compare equality.
  fn eq(&self, other: &Self) -> bool {
    self.label.eq(&other.label)
  }
}

/// A raw window type that contains fields to access
/// the HWND on Windows, gtk::ApplicationWindow on Linux and
/// NSView on macOS.
pub struct RawWindow<'a> {
  #[cfg(windows)]
  pub hwnd: isize,
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  ))]
  pub gtk_window: &'a gtk::ApplicationWindow,
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  ))]
  pub default_vbox: Option<&'a gtk::Box>,
  pub _marker: &'a PhantomData<()>,
}
