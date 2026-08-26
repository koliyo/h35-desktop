from __future__ import annotations

import os
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
from pathlib import Path

from h35_ops.paths import repo_root

PACKAGE_USAGE = (
    "usage: APP_NAME=… BUNDLE_ID=… EXECUTABLE=… CRATE=… [PRODUCT_ROOT=…] "
    "h35-ops package"
)
ASSEMBLE_USAGE = (
    "usage: h35-ops assemble <binary> <dest-app> <version> [bundle-version]"
)
SIGN_USAGE = "usage: h35-ops sign <App.app>"
APPCAST_USAGE = "usage: h35-ops appcast <inbox-dir> <download-url-prefix>"
SIGN_SECRETS = (
    "APPLE_DEVELOPER_ID_APPLICATION",
    "APPLE_API_KEY_ID",
    "APPLE_API_ISSUER",
    "APPLE_API_KEY",
)
DEFAULT_SPARKLE_VERSION = "2.8.1"
VERSION_RE = re.compile(r'^version = "([^"]+)"', re.MULTILINE)


def macos_dir(root: Path | None = None) -> Path:
    return (root or repo_root()) / "packaging" / "macos"


def cargo_version(product_root: Path) -> str:
    text = (product_root / "Cargo.toml").read_text(encoding="utf-8")
    match = VERSION_RE.search(text)
    if not match:
        raise SystemExit("h35-ops package: could not read package.version from Cargo.toml")
    return match.group(1)


def require_env(*names: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for name in names:
        value = os.environ.get(name, "")
        if not value:
            raise SystemExit(f"h35-ops assemble: {name} is required")
        values[name] = value
    return values


def missing_secrets() -> list[str]:
    return [name for name in SIGN_SECRETS if not os.environ.get(name)]


def fetch_sparkle(root: Path | None = None) -> Path:
    root = root or repo_root()
    version = os.environ.get("SPARKLE_VERSION") or DEFAULT_SPARKLE_VERSION
    dest = Path(os.environ.get("SPARKLE_CACHE") or root / "target" / "sparkle" / version)
    framework = dest / "Sparkle.framework"
    if not framework.is_dir():
        dest.mkdir(parents=True, exist_ok=True)
        archive = dest / f"Sparkle-{version}.tar.xz"
        url = (
            "https://github.com/sparkle-project/Sparkle/releases/download/"
            f"{version}/Sparkle-{version}.tar.xz"
        )
        if not archive.is_file():
            urllib.request.urlretrieve(url, archive)
        with tarfile.open(archive, "r:xz") as tf:
            members = [
                member
                for member in tf.getmembers()
                if member.name.startswith("Sparkle.framework") or member.name.startswith("bin/")
            ]
            tf.extractall(dest, members=members, filter="data")
    if not framework.is_dir():
        raise SystemExit(f"h35-ops package: Sparkle.framework missing under {dest}")
    return framework


def _is_macho(path: Path) -> bool:
    result = subprocess.run(
        ["/usr/bin/file", "-b", str(path)],
        check=False,
        capture_output=True,
        text=True,
    )
    return "Mach-O" in result.stdout


def assemble(
    binary: Path,
    dest: Path,
    version: str,
    bundle_version: str | None = None,
    *,
    root: Path | None = None,
) -> None:
    env = require_env("APP_NAME", "BUNDLE_ID", "EXECUTABLE")
    binary = binary.resolve()
    if not binary.is_file():
        raise SystemExit(f"h35-ops assemble: binary not found: {binary}")
    template = Path(os.environ.get("INFO_PLIST") or macos_dir(root) / "Info.plist")
    if not template.is_file():
        raise SystemExit(f"h35-ops assemble: missing {template}")
    bundle_version = bundle_version or version
    if dest.exists():
        shutil.rmtree(dest)
    macos = dest / "Contents" / "MacOS"
    resources = dest / "Contents" / "Resources"
    macos.mkdir(parents=True)
    resources.mkdir(parents=True)
    executable = macos / env["EXECUTABLE"]
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    plist = template.read_text(encoding="utf-8")
    replacements = {
        "@APP_NAME@": env["APP_NAME"],
        "@EXECUTABLE@": env["EXECUTABLE"],
        "@BUNDLE_ID@": env["BUNDLE_ID"],
        "@VERSION@": version,
        "@BUNDLE_VERSION@": bundle_version,
        "@SU_FEED_URL@": os.environ.get("SU_FEED_URL", ""),
        "@SU_PUBLIC_ED_KEY@": os.environ.get("SU_PUBLIC_ED_KEY", ""),
    }
    for needle, value in replacements.items():
        plist = plist.replace(needle, value)
    (dest / "Contents" / "Info.plist").write_text(plist, encoding="utf-8")
    (dest / "Contents" / "PkgInfo").write_bytes(b"APPL????")
    icon = os.environ.get("APP_ICON")
    if icon and Path(icon).is_file():
        shutil.copy2(icon, resources / "AppIcon.icns")
    sparkle = os.environ.get("SPARKLE_FRAMEWORK")
    if sparkle:
        sparkle_path = Path(sparkle)
        if not sparkle_path.is_dir():
            raise SystemExit(
                f"h35-ops assemble: SPARKLE_FRAMEWORK is not a directory: {sparkle}"
            )
        frameworks = dest / "Contents" / "Frameworks"
        frameworks.mkdir(parents=True, exist_ok=True)
        copied = frameworks / "Sparkle.framework"
        if copied.exists():
            shutil.rmtree(copied)
        shutil.copytree(sparkle_path, copied, symlinks=True)
        if platform.system() == "Darwin" and _is_macho(executable):
            subprocess.run(
                [
                    "install_name_tool",
                    "-add_rpath",
                    "@executable_path/../Frameworks",
                    str(executable),
                ],
                check=True,
            )


def package_product() -> int:
    crate = os.environ.get("CRATE", "")
    executable = os.environ.get("EXECUTABLE", "")
    app_name = os.environ.get("APP_NAME", "")
    bundle_id = os.environ.get("BUNDLE_ID", "")
    if not crate or not executable or not app_name or not bundle_id:
        raise SystemExit(PACKAGE_USAGE)
    product_root = Path(os.environ.get("PRODUCT_ROOT") or Path.cwd())
    version = cargo_version(product_root)
    subprocess.run(["cargo", "build", "--release", "-p", crate], cwd=product_root, check=True)
    if platform.system() == "Darwin":
        os.environ["SPARKLE_FRAMEWORK"] = str(fetch_sparkle())
    dest = product_root / "dist" / f"{app_name}.app"
    assemble(
        product_root / "target" / "release" / executable,
        dest,
        version,
        os.environ.get("BUNDLE_VERSION") or version,
    )
    if platform.system() == "Darwin":
        subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(dest)], check=True)
    print(dest)
    return 0


def _codesign_nested(identity: str, path: Path) -> None:
    if path.exists():
        subprocess.run(
            [
                "codesign",
                "--force",
                "--options",
                "runtime",
                "--timestamp",
                "--sign",
                identity,
                str(path),
            ],
            check=True,
        )


def sign_app(app: Path) -> int:
    if not app.is_dir():
        raise SystemExit(f"h35-ops sign: not an app bundle: {app}")
    executable = os.environ.get("EXECUTABLE") or app.name.removesuffix(".app").lower()
    missing = missing_secrets()
    if os.environ.get("SIGN_DRY_RUN") == "1":
        print("h35-ops sign: dry-run (not signing, not submitting to notarytool)")
        if missing:
            print(f"h35-ops sign: would fail-closed without: {' '.join(missing)}")
        else:
            print(
                "h35-ops sign: would codesign with hardened runtime using "
                "APPLE_DEVELOPER_ID_APPLICATION"
            )
            print("h35-ops sign: would notarytool submit and stapler staple")
        return 0
    if missing:
        raise SystemExit(
            "h35-ops sign: missing signing secrets: "
            + " ".join(missing)
            + "\nh35-ops sign: refusing to upload an unsigned production archive"
        )
    identity = os.environ["APPLE_DEVELOPER_ID_APPLICATION"]
    framework = app / "Contents" / "Frameworks" / "Sparkle.framework"
    if framework.is_dir():
        for helper in framework.rglob("*"):
            if helper.name in {"Autoupdate", "Updater"} or helper.suffix == ".xpc":
                _codesign_nested(identity, helper)
        _codesign_nested(identity, framework)
    _codesign_nested(identity, app / "Contents" / "MacOS" / executable)
    _codesign_nested(identity, app)
    with tempfile.TemporaryDirectory() as scratch:
        zip_path = Path(scratch) / "App.zip"
        subprocess.run(["ditto", "-c", "-k", "--keepParent", str(app), str(zip_path)], check=True)
        keyfile = Path(scratch) / "api.p8"
        keyfile.write_text(os.environ["APPLE_API_KEY"] + "\n", encoding="utf-8")
        subprocess.run(
            [
                "xcrun",
                "notarytool",
                "submit",
                str(zip_path),
                "--key",
                str(keyfile),
                "--key-id",
                os.environ["APPLE_API_KEY_ID"],
                "--issuer",
                os.environ["APPLE_API_ISSUER"],
                "--wait",
            ],
            check=True,
        )
        subprocess.run(["xcrun", "stapler", "staple", str(app)], check=True)
    print(f"h35-ops sign: stapled {app}")
    return 0


def generate_appcast(inbox: Path, prefix: str, *, root: Path | None = None) -> int:
    secret = os.environ.get("SPARKLE_EDDSA_PRIVATE_KEY")
    if not secret:
        raise SystemExit("h35-ops appcast: SPARKLE_EDDSA_PRIVATE_KEY is required")
    if not inbox.is_dir():
        raise SystemExit(f"h35-ops appcast: inbox is not a directory: {inbox}")
    if any(child.is_dir() for child in inbox.iterdir()):
        raise SystemExit("h35-ops appcast: inbox must be flat (no subdirectories)")
    tool = os.environ.get("GENERATE_APPCAST")
    if not tool:
        tool = str(fetch_sparkle(root).parent / "bin" / "generate_appcast")
    subprocess.run(
        [
            tool,
            "--maximum-deltas",
            "0",
            "--download-url-prefix",
            prefix,
            "--ed-key-file",
            "-",
            "-o",
            str(inbox / "appcast.xml"),
            str(inbox),
        ],
        input=secret,
        text=True,
        check=True,
    )
    return 0


def package_command(argv: list[str]) -> int:
    if argv:
        raise SystemExit(PACKAGE_USAGE)
    return package_product()


def assemble_command(argv: list[str]) -> int:
    if len(argv) < 3 or len(argv) > 4 or argv[0] in ("-h", "--help"):
        raise SystemExit(ASSEMBLE_USAGE)
    assemble(
        Path(argv[0]),
        Path(argv[1]),
        argv[2],
        argv[3] if len(argv) == 4 else None,
    )
    return 0


def sign_command(argv: list[str]) -> int:
    if len(argv) != 1 or argv[0] in ("-h", "--help"):
        raise SystemExit(SIGN_USAGE)
    return sign_app(Path(argv[0]))


def appcast_command(argv: list[str]) -> int:
    if len(argv) != 2 or argv[0] in ("-h", "--help"):
        raise SystemExit(APPCAST_USAGE)
    return generate_appcast(Path(argv[0]), argv[1])
