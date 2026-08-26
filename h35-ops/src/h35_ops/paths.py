from __future__ import annotations

import os
from pathlib import Path


def repo_root() -> Path:
    env = os.environ.get("H35_DESKTOP_REPO_ROOT")
    if env:
        return Path(env)
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "h35-ops").is_dir() and (parent / "Cargo.toml").is_file():
            return parent
    raise SystemExit("could not find h35-desktop repository root")
