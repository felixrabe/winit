//! The `EventsLoop` struct and assorted supporting types, including `ControlFlow`.

use platform;
use events::Event;
use {AvailableMonitorsIter, MonitorId};

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
///
/// An `EventsLoop` can be seen more or less as a "context". Calling `EventsLoop::new()`
/// initializes everything that will be required to create windows. For example on Linux creating
/// an events loop opens a connection to the X or Wayland server.
///
/// To wake up an `EventsLoop` from a another thread, see the `EventsLoopProxy` docs.
///
/// Note that the `EventsLoop` cannot be shared across threads (due to platform-dependant logic
/// forbidding it), as such it is neither `Send` nor `Sync`. If you need cross-thread access, the
/// `Window` created from this `EventsLoop` _can_ be sent to an other thread, and the
/// `EventsLoopProxy` allows you to wakeup an `EventsLoop` from an other thread.
pub struct EventsLoop {
    pub(crate) events_loop: platform::EventsLoop,
    _marker: ::std::marker::PhantomData<*mut ()> // Not Send nor Sync
}

impl std::fmt::Debug for EventsLoop {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmtr.pad("EventsLoop { .. }")
    }
}

/// Returned by the user callback given to the `EventsLoop::run_forever` method.
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

impl EventsLoop {
    /// Builds a new events loop.
    ///
    /// Usage will result in display backend initialisation, this can be controlled on linux
    /// using an environment variable `WINIT_UNIX_BACKEND`. Legal values are `x11` and `wayland`.
    /// If it is not set, winit will try to connect to a wayland connection, and if it fails will
    /// fallback on x11. If this variable is set with any other value, winit will panic.
    pub fn new() -> EventsLoop {
        EventsLoop {
            events_loop: platform::EventsLoop::new(),
            _marker: ::std::marker::PhantomData,
        }
    }

    /// Returns the list of all the monitors available on the system.
    ///
    // Note: should be replaced with `-> impl Iterator` once stable.
    #[inline]
    pub fn get_available_monitors(&self) -> AvailableMonitorsIter {
        let data = self.events_loop.get_available_monitors();
        AvailableMonitorsIter{ data: data.into_iter() }
    }

    /// Returns the primary monitor of the system.
    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        MonitorId { inner: self.events_loop.get_primary_monitor() }
    }

    /// Fetches all the events that are pending, calls the callback function for each of them,
    /// and returns.
    #[inline]
    pub fn poll_events<F>(&mut self, callback: F)
        where F: FnMut(Event)
    {
        self.events_loop.poll_events(callback)
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
        self.events_loop.run_forever(callback)
    }

    /// Creates an `EventsLoopProxy` that can be used to wake up the `EventsLoop` from another
    /// thread.
    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            events_loop_proxy: self.events_loop.create_proxy(),
        }
    }
}

/// Used to wake up the `EventsLoop` from another thread.
#[derive(Clone)]
pub struct EventsLoopProxy {
    events_loop_proxy: platform::EventsLoopProxy,
}

impl std::fmt::Debug for EventsLoopProxy {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmtr.pad("EventsLoopProxy { .. }")
    }
}

impl EventsLoopProxy {
    /// Wake up the `EventsLoop` from which this proxy was created.
    ///
    /// This causes the `EventsLoop` to emit an `Awakened` event.
    ///
    /// Returns an `Err` if the associated `EventsLoop` no longer exists.
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        self.events_loop_proxy.wakeup()
    }
}

/// The error that is returned when an `EventsLoopProxy` attempts to wake up an `EventsLoop` that
/// no longer exists.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EventsLoopClosed;

impl std::fmt::Display for EventsLoopClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", std::error::Error::description(self))
    }
}

impl std::error::Error for EventsLoopClosed {
    fn description(&self) -> &str {
        "Tried to wake up a closed `EventsLoop`"
    }
}
