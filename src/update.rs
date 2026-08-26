pub fn updater_allowed(bundled: bool, opening_desktop: bool) -> bool {
    bundled && opening_desktop
}

pub struct Handle {
    #[cfg(target_os = "macos")]
    controller: objc2::rc::Retained<objc2::runtime::AnyObject>,
}

pub fn start(allowed: bool) -> Option<Handle> {
    if !allowed {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        macos::start()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

impl Handle {
    pub fn check_for_updates(&self) {
        #[cfg(target_os = "macos")]
        macos::check_for_updates(&self.controller);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::CString;
    use std::path::PathBuf;

    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};

    pub(super) fn start() -> Option<super::Handle> {
        load_sparkle()?;
        let class = AnyClass::get(c"SPUStandardUpdaterController")?;
        let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
        let controller: *mut AnyObject = unsafe {
            msg_send![
                allocated,
                initWithStartingUpdater: true,
                updaterDelegate: std::ptr::null::<AnyObject>(),
                userDriverDelegate: std::ptr::null::<AnyObject>()
            ]
        };
        let controller = unsafe { Retained::from_raw(controller) }?;
        Some(super::Handle { controller })
    }

    pub(super) fn check_for_updates(controller: &Retained<AnyObject>) {
        let _: () =
            unsafe { msg_send![&**controller, checkForUpdates: std::ptr::null::<AnyObject>()] };
    }

    fn load_sparkle() -> Option<()> {
        if AnyClass::get(c"SPUStandardUpdaterController").is_some() {
            return Some(());
        }
        let path = sparkle_dylib()?;
        let c_path = CString::new(path.to_str()?).ok()?;
        let handle = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
        if handle.is_null() {
            return None;
        }
        AnyClass::get(c"SPUStandardUpdaterController").map(|_| ())
    }

    fn sparkle_dylib() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let path = exe
            .parent()?
            .parent()?
            .join("Frameworks/Sparkle.framework/Sparkle");
        path.is_file().then_some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updater_allowed_only_when_bundled_desktop_opens() {
        assert!(updater_allowed(true, true));
        assert!(!updater_allowed(true, false));
        assert!(!updater_allowed(false, true));
        assert!(!updater_allowed(false, false));
    }

    #[test]
    fn start_is_none_when_not_allowed() {
        assert!(start(false).is_none());
    }
}
