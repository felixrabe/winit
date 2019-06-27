//! Winit allows you to build a window on as many platforms as possible.
//!
//! # Building a window
//!
//! Before you can build a window, you first need to build an `EventsLoop`. This is done with the
//! `EventsLoop::new()` function. Example:
//!
//! ```no_run
//! use winit::events_loop::EventsLoop;
//! let events_loop = EventsLoop::new();
//! ```
//!
//! Once this is done there are two ways to create a window:
//!
//!  - Calling `Window::new(&events_loop)`.
//!  - Calling `let builder = WindowBuilder::new()` then `builder.build(&events_loop)`.
//!
//! The first way is the simplest way and will give you default values for everything.
//!
//! The second way allows you to customize the way your window will look and behave by modifying
//! the fields of the `WindowBuilder` object before you create the window.
//!
//! # Events handling
//!
//! Once a window has been created, it will *generate events*. For example whenever the user moves
//! the window, resizes the window, moves the mouse, etc. an event is generated.
//!
//! The events generated by a window can be retrieved from the `EventsLoop` the window was created
//! with.
//!
//! There are two ways to do so. The first is to call `events_loop.poll_events(...)`, which will
//! retrieve all the events pending on the windows and immediately return after no new event is
//! available. You usually want to use this method in application that render continuously on the
//! screen, such as video games.
//!
//! ```no_run
//! use winit::{Event, WindowEvent};
//! use winit::dpi::LogicalSize;
//! # use winit::events_loop::EventsLoop;
//! # let mut events_loop = EventsLoop::new();
//!
//! loop {
//!     events_loop.poll_events(|event| {
//!         match event {
//!             Event::WindowEvent {
//!                 event: WindowEvent::Resized(LogicalSize { width, height }),
//!                 ..
//!             } => {
//!                 println!("The window was resized to {}x{}", width, height);
//!             },
//!             _ => ()
//!         }
//!     });
//! }
//! ```
//!
//! The second way is to call `events_loop.run_forever(...)`. As its name tells, it will run
//! forever unless it is stopped by returning `ControlFlow::Break`.
//!
//! ```no_run
//! use winit::{events_loop::ControlFlow, Event, WindowEvent};
//! # use winit::events_loop::EventsLoop;
//! # let mut events_loop = EventsLoop::new();
//!
//! events_loop.run_forever(|event| {
//!     match event {
//!         Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
//!             println!("The close button was pressed; stopping");
//!             ControlFlow::Break
//!         },
//!         _ => ControlFlow::Continue,
//!     }
//! });
//! ```
//!
//! If you use multiple windows, the `WindowEvent` event has a member named `window_id`. You can
//! compare it with the value returned by the `id()` method of `Window` in order to know which
//! window has received the event.
//!
//! # Drawing on the window
//!
//! Winit doesn't provide any function that allows drawing on a window. However it allows you to
//! retrieve the raw handle of the window (see the `os` module for that), which in turn allows you
//! to create an OpenGL/Vulkan/DirectX/Metal/etc. context that will draw on the window.
//!

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;
extern crate libc;
#[macro_use]
extern crate log;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

#[cfg(target_os = "windows")]
extern crate winapi;
#[cfg(target_os = "windows")]
extern crate backtrace;
#[macro_use]
#[cfg(target_os = "windows")]
extern crate bitflags;
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[macro_use]
extern crate objc;
#[cfg(target_os = "macos")]
extern crate cocoa;
#[cfg(target_os = "macos")]
extern crate core_foundation;
#[cfg(target_os = "macos")]
extern crate core_graphics;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate x11_dl;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate parking_lot;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate percent_encoding;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate smithay_client_toolkit as sctk;

pub(crate) use dpi::*; // TODO: Actually change the imports throughout the codebase.
pub use events::*;
pub use window::{AvailableMonitorsIter, MonitorId};
pub use icon::*;

pub mod dpi;
pub mod events_loop;
mod events;
mod icon;
mod platform_impl;
mod window;

pub mod platform;

/// Represents a window.
///
/// # Example
///
/// ```no_run
/// use winit::{Event, events_loop::EventsLoop, Window, WindowEvent, events_loop::ControlFlow};
///
/// let mut events_loop = EventsLoop::new();
/// let window = Window::new(&events_loop).unwrap();
///
/// events_loop.run_forever(|event| {
///     match event {
///         Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
///             ControlFlow::Break
///         },
///         _ => ControlFlow::Continue,
///     }
/// });
/// ```
pub struct Window {
    window: platform_impl::Window,
}

impl std::fmt::Debug for Window {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmtr.pad("Window { .. }")
    }
}

/// Identifier of a window. Unique for each window.
///
/// Can be obtained with `window.id()`.
///
/// Whenever you receive an event specific to a window, this event contains a `WindowId` which you
/// can then compare to the ids of your windows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(platform_impl::WindowId);

impl WindowId {
    /// Returns a dummy `WindowId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `WindowId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        WindowId(platform_impl::WindowId::dummy())
    }
}

/// Identifier of an input device.
///
/// Whenever you receive an event arising from a particular input device, this event contains a `DeviceId` which
/// identifies its origin. Note that devices may be virtual (representing an on-screen cursor and keyboard focus) or
/// physical. Virtual devices typically aggregate inputs from multiple physical devices.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(platform_impl::DeviceId);

impl DeviceId {
    /// Returns a dummy `DeviceId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `DeviceId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        DeviceId(platform_impl::DeviceId::dummy())
    }
}

/// Object that allows you to build windows.
#[derive(Clone)]
pub struct WindowBuilder {
    /// The attributes to use to create the window.
    pub window: WindowAttributes,

    // Platform-specific configuration. Private.
    platform_specific: platform_impl::PlatformSpecificWindowBuilderAttributes,
}

impl std::fmt::Debug for WindowBuilder {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmtr.debug_struct("WindowBuilder")
            .field("window", &self.window)
            .finish()
    }
}

/// Error that can happen while creating a window or a headless renderer.
#[derive(Debug, Clone)]
pub enum CreationError {
    OsError(String),
    /// TODO: remove this error
    NotSupported,
}

impl CreationError {
    fn to_string(&self) -> &str {
        match *self {
            CreationError::OsError(ref text) => &text,
            CreationError::NotSupported => "Some of the requested attributes are not supported",
        }
    }
}

impl std::fmt::Display for CreationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        formatter.write_str(self.to_string())
    }
}

impl std::error::Error for CreationError {
    fn description(&self) -> &str {
        self.to_string()
    }
}

/// Describes the appearance of the mouse cursor.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MouseCursor {
    /// The platform-dependent default cursor.
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
    Grab,
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

impl Default for MouseCursor {
    fn default() -> Self {
        MouseCursor::Default
    }
}

/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub struct WindowAttributes {
    /// The dimensions of the window. If this is `None`, some platform-specific dimensions will be
    /// used.
    ///
    /// The default is `None`.
    pub dimensions: Option<LogicalSize>,

    /// The minimum dimensions a window can be, If this is `None`, the window will have no minimum dimensions (aside from reserved).
    ///
    /// The default is `None`.
    pub min_dimensions: Option<LogicalSize>,

    /// The maximum dimensions a window can be, If this is `None`, the maximum will have no maximum or will be set to the primary monitor's dimensions by the platform.
    ///
    /// The default is `None`.
    pub max_dimensions: Option<LogicalSize>,

    /// Whether the window is resizable or not.
    ///
    /// The default is `true`.
    pub resizable: bool,

    /// Whether the window should be set as fullscreen upon creation.
    ///
    /// The default is `None`.
    pub fullscreen: Option<MonitorId>,

    /// The title of the window in the title bar.
    ///
    /// The default is `"winit window"`.
    pub title: String,

    /// Whether the window should be maximized upon creation.
    ///
    /// The default is `false`.
    pub maximized: bool,

    /// Whether the window should be immediately visible upon creation.
    ///
    /// The default is `true`.
    pub visible: bool,

    /// Whether the the window should be transparent. If this is true, writing colors
    /// with alpha values different than `1.0` will produce a transparent window.
    ///
    /// The default is `false`.
    pub transparent: bool,

    /// Whether the window should have borders and bars.
    ///
    /// The default is `true`.
    pub decorations: bool,

    /// Whether the window should always be on top of other windows.
    ///
    /// The default is `false`.
    pub always_on_top: bool,

    /// The window icon.
    ///
    /// The default is `None`.
    pub window_icon: Option<Icon>,

    /// [iOS only] Enable multitouch,
    /// see [multipleTouchEnabled](https://developer.apple.com/documentation/uikit/uiview/1622519-multipletouchenabled)
    pub multitouch: bool,
}

impl Default for WindowAttributes {
    #[inline]
    fn default() -> WindowAttributes {
        WindowAttributes {
            dimensions: None,
            min_dimensions: None,
            max_dimensions: None,
            resizable: true,
            title: "winit window".to_owned(),
            maximized: false,
            fullscreen: None,
            visible: true,
            transparent: false,
            decorations: true,
            always_on_top: false,
            window_icon: None,
            multitouch: false,
        }
    }
}
