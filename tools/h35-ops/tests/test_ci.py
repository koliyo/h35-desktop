from h35_ops.ci import JOB_NAMES, parse_ci_args, steps_for
from h35_ops.paths import repo_root


def test_list_jobs_are_stable() -> None:
    assert JOB_NAMES == ("test",)


def test_parse_list_flag() -> None:
    args = parse_ci_args(["--list"])
    assert args.list is True
    assert args.jobs == []


def test_test_job_matches_hosted_ci() -> None:
    argv_lists = [s.argv for s in steps_for("test", repo_root())]
    assert any(argv[:3] == ("cargo", "fmt", "--all") for argv in argv_lists)
    assert any(argv == ("cargo", "test") for argv in argv_lists)
