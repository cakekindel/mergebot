[![crates.io](https://img.shields.io/crates/v/{{PACKAGE}}.svg)](https://crates.io/crates/{{PACKAGE}})
[![docs.rs](https://docs.rs/{{PACKAGE}}/badge.svg)](https://docs.rs/{{PACKAGE}}/latest)
![Maintenance](https://img.shields.io/badge/maintenance-activly--developed-brightgreen.svg)

# mergebot

Orion's Rust crate template.

This includes a CI/CD pipeline, README templating, and cargo-make scripts.

## Setup
|Type|Name|Value|How-To|
|--|--|--|--|
|Github Repo Secret|CARGO_TOKEN|Token issued for your user by crates.io|[How-To](https://doc.rust-lang.org/cargo/reference/publishing.html#before-your-first-publish)|
|Github Repo Secret|GH_TOKEN|A GitHub PAT|[How-To](https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token)|

## cargo-make
This crate uses [`cargo-make`] for script consistency, in Makefile.toml you'll find:
  - `cargo make fmt`: Format all files according to configured style `rustfmt.toml`
  - `cargo make test`: Run all tests
  - `cargo make doctest`: Run doc tests only
  - `cargo make tdd`: Watch files for changes, and run `cargo make test` on each change
  - `cargo make ci`: Run tests, check that code is formatted and no lint violations.
                     This is run as a quality gate for all pull requests.
  - `cargo make update-readme`: Regenerate README.md based on `src/lib.rs` and `./README.tpl`.

## README
Uses [`cargo-readme`] for README generation -
see `src/lib.rs` and `./README.tpl` for actual documentation source.

## CI/CD
> Note: requires following [conventional commits].

On Pull Request -> main:
  - run `cargo make ci` (test && rustfmt --check && clippy)

On Pull Request merge -> main:
  - Uses [`standard-version`] (bump version & update CHANGELOG)
  - Pushes `chore(release): vX.X.X`
  - Pushes tag `vX.X.X`

On tag push:
  - Publish new GitHub release
  - Publish new version to crates.io

[`cargo-make`]: https://github.com/sagiegurari/cargo-make/
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
