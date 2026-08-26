from __future__ import annotations

import argparse
import subprocess
from dataclasses import dataclass
from pathlib import Path

from h35_ops.paths import repo_root

JOB_NAMES = ("test",)


@dataclass(frozen=True)
class Step:
    argv: tuple[str, ...]


def steps_for(job: str, root: Path) -> list[Step]:
    del root
    if job == "test":
        return [
            Step(("cargo", "fmt", "--all", "--", "--check")),
            Step(("cargo", "test")),
        ]
    raise ValueError(f"unknown job: {job}")


def run_step(step: Step, cwd: Path) -> int:
    result = subprocess.run(list(step.argv), cwd=cwd, check=False)
    return result.returncode


def run_job(job: str, cwd: Path) -> int:
    print(f"==> {job}", flush=True)
    for step in steps_for(job, cwd):
        print("+ " + " ".join(step.argv), flush=True)
        code = run_step(step, cwd)
        if code != 0:
            return code
    return 0


def parse_ci_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="h35-ops ci")
    parser.add_argument("-k", "--keep-going", action="store_true")
    parser.add_argument("-l", "--list", action="store_true")
    parser.add_argument("jobs", nargs="*", choices=JOB_NAMES)
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_ci_args(argv)
    if args.list:
        for name in JOB_NAMES:
            print(name)
        return 0
    jobs = args.jobs or list(JOB_NAMES)
    cwd = repo_root()
    failed: list[str] = []
    for job in jobs:
        code = run_job(job, cwd)
        if code != 0:
            failed.append(job)
            if not args.keep_going:
                return code
    return 1 if failed else 0
