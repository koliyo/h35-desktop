from h35_ops.cli import USAGE


def test_usage_lists_ci_promote_package_and_pr_checkout() -> None:
    assert "ci            run GitHub Actions validation jobs on this machine" in USAGE
    assert "pr-checkout   list open PRs, or checkout one here as pr/<branch>" in USAGE
    assert "promote       tag" in USAGE
    assert "package       product .app from APP_NAME/BUNDLE_ID/EXECUTABLE/CRATE" in USAGE
    assert "assemble      binary dest-app version [bundle-version]" in USAGE
    assert "sign          App.app" in USAGE
    assert "appcast       inbox-dir download-url-prefix" in USAGE
