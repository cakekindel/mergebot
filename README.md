[![crates.io](https://img.shields.io/crates/v/{{PACKAGE}}.svg)](https://crates.io/crates/{{PACKAGE}})
[![docs.rs](https://docs.rs/{{PACKAGE}}/badge.svg)](https://docs.rs/{{PACKAGE}}/latest)
![Maintenance](https://img.shields.io/badge/maintenance-activly--developed-brightgreen.svg)

# mergebot

## mergebot
I'm a slack app triggers approvable deployments for multi-repo applications containing per-environment git branches.

e.g.
- `github.com/todos-app/backend`
  - `main` Prod
  - `qa` QA
  - `staging` Staging
- `github.com/todos-app/frontend`
  - `main` Prod
  - `qa` QA
  - `staging` Staging

## Flow
- User A issues `/deploy foo staging`
- mergebot checks Deployables (configured via `./deployables.json`, which is ignored from source control) for name == "foo"
- mergebot checks `foo.repos` for `environments` matching the name "staging"
- mergebot ensures User A is in `staging.users`
- mergebot queues a merge job for all repos who have a "staging" environment
- mergebot sends a slack message targeting all users with `approver == true` & all user groups asking for approval
- mergebot waits until the users mentioned above have all reacted with :+1:
- when approval conditions met, mergebot executes merge job (`git switch <target>; git merge <base> --no-edit --ff-only --no-verify; git push --no-verify;`)

## Setup
Requirements:
 - [`cargo-make`]
 - [`ngrok`]
 - A git repo with multiple branches (_not_ this one!) for testing
 - A `./deployables.json` file that looks something like `./deployables.example.json`

1. Start a tunnel with `ngrok http 3030` - URL yielded will be referred to as `<ngrok>`
1. Create a slack app with:
   - Scopes: `['chat:write', 'commands', 'reactions:read']`
   - Redirect URI: `<ngrok>/redirect`
   - Slash command: `/deploy` -> `<ngrok>/api/v1/command`
1. Install to a slack workspace
1.

Create a slack app with:
- Redirect URI: ``

## cargo-make
This crate uses [`cargo-make`] for script consistency, in Makefile.toml you'll find:
  - `cargo make fmt`: Format all files according to configured style `rustfmt.toml`
  - `cargo make test`: Run all tests
  - `cargo make doctest`: Run doc tests only
  - `cargo make tdd`: Watch files for changes, and run `cargo make test` on each change
  - `cargo make ci`: Run tests, check that code is formatted and no lint violations.
                     This is run as a quality gate for all pull requests.
  - `cargo make update-readme`: Regenerate README.md based on `src/lib.rs` and `./README.tpl`.

[`cargo-make`]: https://github.com/sagiegurari/cargo-make/
[`ngrok`]: https://ngrok.com/
[`cargo-readme`]: https://github.com/livioribeiro/cargo-readme
[`standard-version`]: https://www.npmjs.com/package/standard-version
[conventional commits]: https://www.conventionalcommits.org/en/v1.0.0/

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
