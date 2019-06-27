//! The `EventLoop` struct and assorted supporting types, including `ControlFlow`.
use std::{fmt, error};

use platform_impl;
use event::Event;
use {AvailableMonitorsIter, MonitorHandle};

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
///
/// An `EventLoop` can be seen more or less as a "context". Calling `EventLoop::new()`
/// initializes everything that will be required to create windows. For example on Linux creating
/// an events loop opens a connection to the X or Wayland server.
///
/// To wake up an `EventLoop` from a another thread, see the `EventLoopProxy` docs.
///
/// Note that the `EventLoop` cannot be shared across threads (due to platform-dependant logic
/// forbidding it), as such it is neither `Send` nor `Sync`. If you need cross-thread access, the
/// `Window` created from this `EventLoop` _can_ be sent to an other thread, and the
/// `EventLoopProxy` allows you to wakeup an `EventLoop` from an other thread.
pub struct EventLoop {
    pub(crate) event_loop: platform_impl::EventLoop,
    _marker: ::std::marker::PhantomData<*mut ()> // Not Send nor Sync
}

impl fmt::Debug for EventLoop {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmtr.pad("EventLoop { .. }")
    }
}

/// Returned by the user callback given to the `EventLoop::run_forever` method.
///
/// Indicates whether the `run_forever` method should continue or complete.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ControlFlow {
    /// Continue looping and waiting for events.
    Continue,
    /// Break from the event loop.
    Break,
}

impl EventLoop {
    /// Builds a new events loop.
    ///
    /// Usage will result in display backend initialisation, this can be controlled on linux
    /// using an environment variable `WINIT_UNIX_BACKEND`. Legal values are `x11` and `wayland`.
    /// If it is not set, winit will try to connect to a wayland connection, and if it fails will
    /// fallback on x11. If this variable is set with any other value, winit will panic.
    pub fn new() -> EventLoop {
        EventLoop {
            event_loop: platform_impl::EventLoop::new(),
            _marker: ::std::marker::PhantomData,
        }
    }

    /// Returns the list of all the monitors available on the system.
    ///
    // Note: should be replaced with `-> impl Iterator` once stable.
    #[inline]
    pub fn get_available_monitors(&self) -> AvailableMonitorsIter {
        let data = self.event_loop.get_available_monitors();
        AvailableMonitorsIter{ data: data.into_iter() }
    }

    /// Returns the primary monitor of the system.
    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle { inner: self.event_loop.get_primary_monitor() }
    }

    /// Fetches all the events that are pending, calls the callback function for each of them,
    /// and returns.
    #[inline]
    pub fn poll_events<F>(&mut self, callback: F)
        where F: FnMut(Event)
    {
        self.event_loop.poll_events(callback)
    }

    /// Calls `callback` every time an event is received. If no event is available, sleeps the
    /// current thread and waits for an event. If the callback returns `ControlFlow::Break` then
    /// `run_forever` will immediately return.
    ///
    /// # Danger!
    ///
    /// The callback is run after *every* event, so if its execution time is non-trivial the event queue may not empty
    /// at a sufficient rate. Rendering in the callback with vsync enabled **will** cause significant lag.
    #[inline]
    pub fn run_forever<F>(&mut self, callback: F)
        where F: FnMut(Event) -> ControlFlow
    {
        self.event_loop.run_forever(callback)
    }

    /// Creates an `EventLoopProxy` that can be used to wake up the `EventLoop` from another
    /// thread.
    pub fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy {
            event_loop_proxy: self.event_loop.create_proxy(),
        }
    }
}

/// Used to wake up the `EventLoop` from another thread.
#[derive(Clone)]
pub struct EventLoopProxy {
    event_loop_proxy: platform_impl::EventLoopProxy,
}

impl fmt::Debug for EventLoopProxy {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmtr.pad("EventLoopProxy { .. }")
    }
}

impl EventLoopProxy {
    /// Wake up the `EventLoop` from which this proxy was created.
    ///
    /// This causes the `EventLoop` to emit an `Awakened` event.
    ///
    /// Returns an `Err` if the associated `EventLoop` no longer exists.
    pub fn wakeup(&self) -> Result<(), EventLoopClosed> {
        self.event_loop_proxy.wakeup()
    }
}

/// The error that is returned when an `EventLoopProxy` attempts to wake up an `EventLoop` that
/// no longer exists.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EventLoopClosed;

impl fmt::Display for EventLoopClosed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", error::Error::description(self))
    }
}

impl error::Error for EventLoopClosed {
    fn description(&self) -> &str {
        "Tried to wake up a closed `EventLoop`"
    }
}
