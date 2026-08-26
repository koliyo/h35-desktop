# h35-desktop agent instructions

## Validate proportionally

- `cargo fmt --all -- --check`
- `cargo test`
- When `h35-ops` is present: `uv run h35-ops ci`.

## Rolling `dev` tag

A later `git pull` may report `! [rejected] dev -> dev (would clobber existing
tag)` after `uv run h35-ops promote tag dev`. That is expected for the
movable prerelease tag. Configure this clone to force-update **only** `dev`:

```sh
git config --local --add remote.origin.fetch '+refs/tags/dev:refs/tags/dev'
```

Do not force-fetch all tags; `v*` releases stay immutable. One-shot without
config: `git fetch origin tag dev --force`. Do not treat a rejected `dev` fetch
as a repository error.
