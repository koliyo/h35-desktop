from pathlib import Path

from h35_ops.macos import (
    APPCAST_USAGE,
    ASSEMBLE_USAGE,
    PACKAGE_USAGE,
    SIGN_USAGE,
    appcast_command,
    assemble,
    assemble_command,
    cargo_version,
    generate_appcast,
    package_command,
    sign_app,
    sign_command,
    sparkle_archive_members,
)


def test_package_usage() -> None:
    try:
        package_command(["extra"])
    except SystemExit as exc:
        assert str(exc) == PACKAGE_USAGE
    else:
        raise AssertionError("expected SystemExit")


def test_assemble_usage() -> None:
    try:
        assemble_command(["only-binary"])
    except SystemExit as exc:
        assert str(exc) == ASSEMBLE_USAGE
    else:
        raise AssertionError("expected SystemExit")


def test_sign_usage() -> None:
    try:
        sign_command([])
    except SystemExit as exc:
        assert str(exc) == SIGN_USAGE
    else:
        raise AssertionError("expected SystemExit")


def test_appcast_usage() -> None:
    try:
        appcast_command(["inbox"])
    except SystemExit as exc:
        assert str(exc) == APPCAST_USAGE
    else:
        raise AssertionError("expected SystemExit")


def test_sparkle_archive_members_accepts_dot_slash_prefix() -> None:
    class Member:
        def __init__(self, name: str) -> None:
            self.name = name

    selected = sparkle_archive_members(
        [
            Member("./LICENSE"),
            Member("./bin/generate_appcast"),
            Member("./Sparkle.framework/Sparkle"),
            Member("Sparkle.framework/Versions/B/Sparkle"),
            Member("./sparkle.app/Contents/MacOS/sparkle"),
        ]
    )
    assert [member.name for member in selected] == [
        "./bin/generate_appcast",
        "./Sparkle.framework/Sparkle",
        "Sparkle.framework/Versions/B/Sparkle",
    ]


def test_cargo_version(tmp_path: Path) -> None:
    (tmp_path / "Cargo.toml").write_text('[package]\nversion = "9.8.7"\n', encoding="utf-8")
    assert cargo_version(tmp_path) == "9.8.7"


def test_assemble_writes_parameterized_plist(monkeypatch, tmp_path: Path) -> None:
    template = tmp_path / "Info.plist"
    template.write_text(
        "@APP_NAME@ @EXECUTABLE@ @BUNDLE_ID@ @VERSION@ @BUNDLE_VERSION@ "
        "@SU_FEED_URL@ @SU_PUBLIC_ED_KEY@",
        encoding="utf-8",
    )
    binary = tmp_path / "widget"
    binary.write_bytes(b"widget-bin")
    dest = tmp_path / "Widget.app"
    monkeypatch.setenv("APP_NAME", "Widget")
    monkeypatch.setenv("BUNDLE_ID", "com.example.widget")
    monkeypatch.setenv("EXECUTABLE", "widget")
    monkeypatch.setenv("SU_FEED_URL", "https://example.test/appcast.xml")
    monkeypatch.setenv("SU_PUBLIC_ED_KEY", "public")
    monkeypatch.setenv("INFO_PLIST", str(template))
    assemble(binary, dest, "1.2.3", "10")
    plist = (dest / "Contents" / "Info.plist").read_text(encoding="utf-8")
    assert "com.example.widget" in plist
    assert "1.2.3" in plist
    assert "10" in plist
    assert "https://example.test/appcast.xml" in plist
    assert "@APP_NAME@" not in plist
    assert (dest / "Contents" / "MacOS" / "widget").read_bytes() == b"widget-bin"
    assert (dest / "Contents" / "PkgInfo").read_bytes() == b"APPL????"


def test_generate_appcast_is_flat_stdin_and_silent(monkeypatch, tmp_path: Path, capsys) -> None:
    inbox = tmp_path / "inbox"
    inbox.mkdir()
    (inbox / "App.zip").write_bytes(b"zip")
    tool = tmp_path / "generate_appcast"
    tool.write_text(
        "#!/bin/sh\nset -eu\nprintf 'args=%s\\n' \"$*\" > \"$0.out\"\ncat > \"$0.in\"\n"
        "out=\"\"\nprev=\"\"\nfor arg in \"$@\"; do\n"
        "  if [ \"$prev\" = \"-o\" ]; then out=$arg; fi\n"
        "  prev=$arg\ndone\n"
        "printf '%s\\n' "
        "'<enclosure sparkle:edSignature=\"sig\" length=\"1\" type=\"application/octet-stream\"/>' "
        "> \"$out\"\n",
        encoding="utf-8",
    )
    tool.chmod(0o755)
    secret = "unit-test-eddsa-private-key"
    monkeypatch.setenv("GENERATE_APPCAST", str(tool))
    monkeypatch.setenv("SPARKLE_EDDSA_PRIVATE_KEY", secret)
    assert generate_appcast(inbox, "https://example.test/download/v1/") == 0
    captured = capsys.readouterr()
    assert secret not in captured.out
    assert secret not in captured.err
    args = Path(f"{tool}.out").read_text(encoding="utf-8")
    assert "--maximum-deltas 0" in args
    assert "--ed-key-file -" in args
    assert Path(f"{tool}.in").read_text(encoding="utf-8") == secret


def test_generate_appcast_fails_without_ed_signature(monkeypatch, tmp_path: Path) -> None:
    inbox = tmp_path / "inbox"
    inbox.mkdir()
    (inbox / "App.zip").write_bytes(b"zip")
    tool = tmp_path / "generate_appcast"
    tool.write_text(
        "#!/bin/sh\nout=\"\"\nprev=\"\"\nfor arg in \"$@\"; do\n"
        "  if [ \"$prev\" = \"-o\" ]; then out=$arg; fi\n"
        "  prev=$arg\ndone\n"
        "printf '%s\\n' '<enclosure url=\"https://example.test/App.zip\"/>' > \"$out\"\n",
        encoding="utf-8",
    )
    tool.chmod(0o755)
    monkeypatch.setenv("GENERATE_APPCAST", str(tool))
    monkeypatch.setenv("SPARKLE_EDDSA_PRIVATE_KEY", "unit-test-eddsa-private-key")
    try:
        generate_appcast(inbox, "https://example.test/download/v1/")
    except SystemExit as exc:
        assert "missing sparkle:edSignature" in str(exc)
    else:
        raise AssertionError("expected SystemExit")


def test_generate_appcast_fails_on_key_mismatch(monkeypatch, tmp_path: Path) -> None:
    inbox = tmp_path / "inbox"
    inbox.mkdir()
    (inbox / "App.zip").write_bytes(b"zip")
    tool = tmp_path / "generate_appcast"
    tool.write_text(
        "#!/bin/sh\n"
        "printf '%s\\n' 'Warning: SUPublicEDKey does not match key EdDSA in the Keychain'\n"
        "out=\"\"\nprev=\"\"\nfor arg in \"$@\"; do\n"
        "  if [ \"$prev\" = \"-o\" ]; then out=$arg; fi\n"
        "  prev=$arg\ndone\n"
        "printf '%s\\n' "
        "'<enclosure sparkle:edSignature=\"sig\" length=\"1\" type=\"application/octet-stream\"/>' "
        "> \"$out\"\n",
        encoding="utf-8",
    )
    tool.chmod(0o755)
    monkeypatch.setenv("GENERATE_APPCAST", str(tool))
    monkeypatch.setenv("SPARKLE_EDDSA_PRIVATE_KEY", "unit-test-eddsa-private-key")
    try:
        generate_appcast(inbox, "https://example.test/download/v1/")
    except SystemExit as exc:
        assert "does not match SPARKLE_EDDSA_PRIVATE_KEY" in str(exc)
    else:
        raise AssertionError("expected SystemExit")


def test_sign_fails_closed_without_secrets(monkeypatch, tmp_path: Path) -> None:
    app = tmp_path / "App.app"
    app.mkdir()
    monkeypatch.delenv("SIGN_DRY_RUN", raising=False)
    monkeypatch.delenv("APPLE_DEVELOPER_ID_APPLICATION", raising=False)
    monkeypatch.delenv("APPLE_API_KEY_ID", raising=False)
    monkeypatch.delenv("APPLE_API_ISSUER", raising=False)
    monkeypatch.delenv("APPLE_API_KEY", raising=False)
    try:
        sign_app(app)
    except SystemExit as exc:
        assert "missing signing secrets" in str(exc)
    else:
        raise AssertionError("expected SystemExit")


def test_sign_dry_run_does_not_claim_notarization(monkeypatch, tmp_path: Path, capsys) -> None:
    app = tmp_path / "App.app"
    app.mkdir()
    monkeypatch.setenv("SIGN_DRY_RUN", "1")
    monkeypatch.delenv("APPLE_DEVELOPER_ID_APPLICATION", raising=False)
    assert sign_app(app) == 0
    out = capsys.readouterr().out
    assert "dry-run" in out
    assert "stapled" not in out.lower()
