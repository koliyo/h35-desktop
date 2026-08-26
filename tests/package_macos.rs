use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "h35-desktop-{}-{}-{}",
        name,
        std::process::id(),
        nonce
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn assemble_script_writes_parameterized_plist() {
    let root = env!("CARGO_MANIFEST_DIR");
    let dir = scratch("assemble");
    let binary = dir.join("widget");
    fs::write(&binary, b"widget-bin").unwrap();
    let dest = dir.join("Widget.app");
    let status = Command::new(format!("{root}/packaging/macos/assemble.sh"))
        .env("APP_NAME", "Widget")
        .env("BUNDLE_ID", "com.example.widget")
        .env("EXECUTABLE", "widget")
        .env("SU_FEED_URL", "https://example.test/appcast.xml")
        .env("SU_PUBLIC_ED_KEY", "public")
        .arg(&binary)
        .arg(&dest)
        .arg("1.2.3")
        .arg("10")
        .status()
        .unwrap();
    assert!(status.success());
    let plist = fs::read_to_string(dest.join("Contents/Info.plist")).unwrap();
    assert!(plist.contains("com.example.widget"), "{plist}");
    assert!(plist.contains("<string>1.2.3</string>"), "{plist}");
    assert!(plist.contains("<string>10</string>"), "{plist}");
    assert!(
        plist.contains("https://example.test/appcast.xml"),
        "{plist}"
    );
    assert!(!plist.contains("@APP_NAME@"), "{plist}");
    assert_eq!(
        fs::read(dest.join("Contents/MacOS/widget")).unwrap(),
        b"widget-bin"
    );
}

#[test]
fn generate_appcast_helper_is_flat_stdin_and_silent() {
    let root = env!("CARGO_MANIFEST_DIR");
    let dir = scratch("appcast");
    let inbox = dir.join("inbox");
    fs::create_dir_all(&inbox).unwrap();
    fs::write(inbox.join("App.zip"), b"zip").unwrap();
    let tool = dir.join("generate_appcast");
    fs::write(
        &tool,
        "#!/bin/sh\nset -eu\nprintf 'args=%s\\n' \"$*\" > \"$0.out\"\ncat > \"$0.in\"\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&tool).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tool, perms).unwrap();
    let secret = "unit-test-eddsa-private-key";
    let output = Command::new(format!("{root}/packaging/macos/generate-appcast.sh"))
        .env("GENERATE_APPCAST", &tool)
        .env("SPARKLE_EDDSA_PRIVATE_KEY", secret)
        .arg(&inbox)
        .arg("https://example.test/download/v1/")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(!stdout.contains(secret), "{stdout}");
    assert!(!stderr.contains(secret), "{stderr}");
    let args = fs::read_to_string(format!("{}.out", tool.display())).unwrap();
    assert!(args.contains("--maximum-deltas 0"), "{args}");
    assert!(args.contains("--ed-key-file -"), "{args}");
}

#[test]
fn sign_script_fails_closed_without_secrets() {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new(format!("{root}/packaging/macos/sign.sh"))
        .env_remove("SIGN_DRY_RUN")
        .env_remove("APPLE_DEVELOPER_ID_APPLICATION")
        .env_remove("APPLE_API_KEY_ID")
        .env_remove("APPLE_API_ISSUER")
        .env_remove("APPLE_API_KEY")
        .arg(format!("{root}/packaging/macos"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing signing secrets"), "{stderr}");
}

#[test]
fn sign_script_dry_run_does_not_claim_notarization() {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new(format!("{root}/packaging/macos/sign.sh"))
        .env("SIGN_DRY_RUN", "1")
        .env_remove("APPLE_DEVELOPER_ID_APPLICATION")
        .arg(format!("{root}/packaging/macos"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("dry-run"), "{stdout}");
    assert!(!stdout.to_ascii_lowercase().contains("stapled"));
}
