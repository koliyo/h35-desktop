from __future__ import annotations

import sys

from h35_ops import ci, macos, pr_checkout, promote

USAGE = """\
usage: h35-ops <command> [args...]

commands:
  appcast       inbox-dir download-url-prefix
  assemble      binary dest-app version [bundle-version]
  ci            run GitHub Actions validation jobs on this machine
  package       product .app from APP_NAME/BUNDLE_ID/EXECUTABLE/CRATE
  pr-checkout   list open PRs, or checkout one here as pr/<branch>
  promote       tag
  sign          App.app
"""


def main(argv: list[str] | None = None) -> None:
    args = sys.argv[1:] if argv is None else argv
    if not args or args[0] in ("-h", "--help"):
        sys.stdout.write(USAGE)
        if not args:
            raise SystemExit(2)
        raise SystemExit(0)
    command, rest = args[0], args[1:]
    if command == "appcast":
        raise SystemExit(macos.appcast_command(rest))
    if command == "assemble":
        raise SystemExit(macos.assemble_command(rest))
    if command == "ci":
        raise SystemExit(ci.main(rest))
    if command == "package":
        raise SystemExit(macos.package_command(rest))
    if command == "pr-checkout":
        raise SystemExit(pr_checkout.main(rest))
    if command == "promote":
        raise SystemExit(promote.promote_command(rest))
    if command == "sign":
        raise SystemExit(macos.sign_command(rest))
    sys.stderr.write(f"unknown command: {command}\n")
    sys.stderr.write(USAGE)
    raise SystemExit(2)
