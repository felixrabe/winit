use std::os::raw::*;

use parking_lot::Mutex;

use super::{
    ffi::{
        RRCrtcChangeNotifyMask, RROutputPropertyNotifyMask, RRScreenChangeNotifyMask, True, Window,
        XRRScreenResources,
    },
    util, XConnection, XError,
};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    monitor::VideoMode,
};

// Used to test XRandR < 1.5 code path. This should always be committed as false.
const FORCE_RANDR_COMPAT: bool = false;
// Also used for testing. This should always be committed as false.
const DISABLE_MONITOR_LIST_CACHING: bool = false;

lazy_static! {
    static ref XRANDR_VERSION: Mutex<Option<(c_int, c_int)>> = Mutex::default();
    static ref MONITORS: Mutex<Option<Vec<MonitorHandle>>> = Mutex::default();
}

fn version_is_at_least(major: c_int, minor: c_int) -> bool {
    if let Some((avail_major, avail_minor)) = *XRANDR_VERSION.lock() {
        if avail_major == major {
            avail_minor >= minor
        } else {
            avail_major > major
        }
    } else {
        unreachable!();
    }
}

pub fn invalidate_cached_monitor_list() -> Option<Vec<MonitorHandle>> {
    // We update this lazily.
    (*MONITORS.lock()).take()
}

#[derive(Debug, Clone)]
pub struct MonitorHandle {
    /// The actual id
    id: u32,
    /// The name of the monitor
    pub(crate) name: String,
    /// The size of the monitor
    dimensions: (u32, u32),
    /// The position of the monitor in the X screen
    position: (i32, i32),
    /// If the monitor is the primary one
    primary: bool,
    /// The DPI scale factor
    pub(crate) hidpi_factor: f64,
    /// Used to determine which windows are on this monitor
    pub(crate) rect: util::AaRect,
    /// Supported video modes on this monitor
    video_modes: Vec<VideoMode>,
}

impl MonitorHandle {
    fn from_repr(
        xconn: &XConnection,
        resources: *mut XRRScreenResources,
        id: u32,
        repr: util::MonitorRepr,
        primary: bool,
    ) -> Option<Self> {
        let (name, hidpi_factor, video_modes) = unsafe { xconn.get_output_info(resources, &repr)? };
        let (dimensions, position) = unsafe { (repr.size(), repr.position()) };
        let rect = util::AaRect::new(position, dimensions);
        Some(MonitorHandle {
            id,
            name,
            hidpi_factor,
            dimensions,
            position,
            primary,
            rect,
            video_modes,
        })
    }

    pub fn name(&self) -> Option<String> {
        Some(self.name.clone())
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        self.id as u32
    }

    pub fn size(&self) -> PhysicalSize {
        self.dimensions.into()
    }

    pub fn position(&self) -> PhysicalPosition {
        self.position.into()
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        self.video_modes.clone().into_iter()
    }
}

impl XConnection {
    pub fn get_monitor_for_window(&self, window_rect: Option<util::AaRect>) -> MonitorHandle {
        let monitors = self.available_monitors();
        let default = monitors
            .get(0)
            .expect("[winit] Failed to find any monitors using XRandR.");

        let window_rect = match window_rect {
            Some(rect) => rect,
            None => return default.to_owned(),
        };

        let mut largest_overlap = 0;
        let mut matched_monitor = default;
        for monitor in &monitors {
            let overlapping_area = window_rect.get_overlapping_area(&monitor.rect);
            if overlapping_area > largest_overlap {
                largest_overlap = overlapping_area;
                matched_monitor = &monitor;
            }
        }

        matched_monitor.to_owned()
    }

    fn query_monitor_list(&self) -> Vec<MonitorHandle> {
        unsafe {
            let root = (self.xlib.XDefaultRootWindow)(self.display);
            let resources = if version_is_at_least(1, 3) {
                (self.xrandr.XRRGetScreenResourcesCurrent)(self.display, root)
            } else {
                // WARNING: this function is supposedly very slow, on the order of hundreds of ms.
                // Upon failure, `resources` will be null.
                (self.xrandr.XRRGetScreenResources)(self.display, root)
            };

            if resources.is_null() {
                panic!("[winit] `XRRGetScreenResources` returned NULL. That should only happen if the root window doesn't exist.");
            }

            let mut available;
            let mut has_primary = false;

            if self.xrandr_1_5.is_some() && version_is_at_least(1, 5) && !FORCE_RANDR_COMPAT {
                // We're in XRandR >= 1.5, enumerate monitors. This supports things like MST and
                // videowalls.
                let xrandr_1_5 = self.xrandr_1_5.as_ref().unwrap();
                let mut monitor_count = 0;
                let monitors =
                    (xrandr_1_5.XRRGetMonitors)(self.display, root, 1, &mut monitor_count);
                assert!(monitor_count >= 0);
                available = Vec::with_capacity(monitor_count as usize);
                for monitor_index in 0..monitor_count {
                    let monitor = monitors.offset(monitor_index as isize);
                    let is_primary = (*monitor).primary != 0;
                    has_primary |= is_primary;
                    MonitorHandle::from_repr(
                        self,
                        resources,
                        monitor_index as u32,
                        monitor.into(),
                        is_primary,
                    )
                    .map(|monitor_id| available.push(monitor_id));
                }
                (xrandr_1_5.XRRFreeMonitors)(monitors);
            } else {
                // We're in XRandR < 1.5, enumerate CRTCs. Everything will work except MST and
                // videowall setups will also show monitors that aren't in the logical groups the user
                // cares about.
                let primary = (self.xrandr.XRRGetOutputPrimary)(self.display, root);
                available = Vec::with_capacity((*resources).ncrtc as usize);
                for crtc_index in 0..(*resources).ncrtc {
                    let crtc_id = *((*resources).crtcs.offset(crtc_index as isize));
                    let crtc = (self.xrandr.XRRGetCrtcInfo)(self.display, resources, crtc_id);
                    let is_active = (*crtc).width > 0 && (*crtc).height > 0 && (*crtc).noutput > 0;
                    if is_active {
                        let crtc = util::MonitorRepr::from(crtc);
                        let is_primary = crtc.get_output() == primary;
                        has_primary |= is_primary;
                        MonitorHandle::from_repr(self, resources, crtc_id as u32, crtc, is_primary)
                            .map(|monitor_id| available.push(monitor_id));
                    }
                    (self.xrandr.XRRFreeCrtcInfo)(crtc);
                }
            }

            // If no monitors were detected as being primary, we just pick one ourselves!
            if !has_primary {
                if let Some(ref mut fallback) = available.first_mut() {
                    // Setting this here will come in handy if we ever add an `is_primary` method.
                    fallback.primary = true;
                }
            }

            (self.xrandr.XRRFreeScreenResources)(resources);
            available
        }
    }

    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        let mut monitors_lock = MONITORS.lock();
        (*monitors_lock)
            .as_ref()
            .cloned()
            .or_else(|| {
                let monitors = Some(self.query_monitor_list());
                if !DISABLE_MONITOR_LIST_CACHING {
                    (*monitors_lock) = monitors.clone();
                }
                monitors
            })
            .unwrap()
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        self.available_monitors()
            .into_iter()
            .find(|monitor| monitor.primary)
            .expect("[winit] Failed to find any monitors using XRandR.")
    }

    pub fn select_xrandr_input(&self, root: Window) -> Result<c_int, XError> {
        {
            let mut version_lock = XRANDR_VERSION.lock();
            if version_lock.is_none() {
                let mut major = 0;
                let mut minor = 0;
                let has_extension =
                    unsafe { (self.xrandr.XRRQueryVersion)(self.display, &mut major, &mut minor) };
                if has_extension != True {
                    panic!("[winit] XRandR extension not available.");
                }
                *version_lock = Some((major, minor));
            }
        }

        let mut event_offset = 0;
        let mut error_offset = 0;
        let status = unsafe {
            (self.xrandr.XRRQueryExtension)(self.display, &mut event_offset, &mut error_offset)
        };

        if status != True {
            self.check_errors()?;
            unreachable!("[winit] `XRRQueryExtension` failed but no error was received.");
        }

        let mask = RRCrtcChangeNotifyMask | RROutputPropertyNotifyMask | RRScreenChangeNotifyMask;
        unsafe { (self.xrandr.XRRSelectInput)(self.display, root, mask) };

        Ok(event_offset)
    }
}
