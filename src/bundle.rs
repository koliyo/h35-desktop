use std::ffi::OsStr;
use std::path::Path;

pub fn running_inside_app_bundle(exe: &Path) -> bool {
    running_inside_named_app_bundle(exe, None)
}

pub fn running_inside_named_app_bundle(exe: &Path, stem: Option<&str>) -> bool {
    let macos = exe.parent();
    let contents = macos.and_then(Path::parent);
    let app = contents.and_then(Path::parent);
    matches!(
        (
            macos.and_then(|path| path.file_name()),
            contents.and_then(|path| path.file_name()),
            app.and_then(|path| path.file_stem()),
            app.and_then(|path| path.extension()),
        ),
        (Some(macos), Some(contents), Some(name), Some(ext))
            if macos == OsStr::new("MacOS")
                && contents == OsStr::new("Contents")
                && ext == OsStr::new("app")
                && stem.is_none_or(|want| name == OsStr::new(want))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_any_app_layout() {
        let exe = PathBuf::from("/Applications/Widget.app/Contents/MacOS/widget");
        assert!(running_inside_app_bundle(&exe));
        assert!(running_inside_named_app_bundle(&exe, Some("Widget")));
        assert!(!running_inside_named_app_bundle(&exe, Some("Other")));
    }

    #[test]
    fn ignores_plain_binaries() {
        assert!(!running_inside_app_bundle(Path::new(
            "/usr/local/bin/widget"
        )));
    }
}
