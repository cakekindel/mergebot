//! Orion's Rust crate template.
//!
//! This includes a CI/CD pipeline, README templating, and cargo-make scripts.
//!
//! # Setup
//! |Type|Name|Value|How-To|
//! |--|--|--|--|
//! |Github Repo Secret|CARGO_TOKEN|Token issued for your user by crates.io|[How-To](https://doc.rust-lang.org/cargo/reference/publishing.html#before-your-first-publish)|
//! |Github Repo Secret|GH_TOKEN|A GitHub PAT|[How-To](https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token)|
//!
//! # cargo-make
//! This crate uses [`cargo-make`] for script consistency, in Makefile.toml you'll find:
//!   - `cargo make fmt`: Format all files according to configured style `rustfmt.toml`
//!   - `cargo make test`: Run all tests
//!   - `cargo make doctest`: Run doc tests only
//!   - `cargo make tdd`: Watch files for changes, and run `cargo make test` on each change
//!   - `cargo make ci`: Run tests, check that code is formatted and no lint violations.
//!                      This is run as a quality gate for all pull requests.
//!   - `cargo make update-readme`: Regenerate README.md based on `src/lib.rs` and `./README.tpl`.
//!
//! # README
//! Uses [`cargo-readme`] for README generation -
//! see `src/lib.rs` and `./README.tpl` for actual documentation source.
//!
//! # CI/CD
//! > Note: requires following [conventional commits].
//!
//! On Pull Request -> main:
//!   - run `cargo make ci` (test && rustfmt --check && clippy)
//!
//! On Pull Request merge -> main:
//!   - Uses [`standard-version`] (bump version & update CHANGELOG)
//!   - Pushes `chore(release): vX.X.X`
//!   - Pushes tag `vX.X.X`
//!
//! On tag push:
//!   - Publish new GitHub release
//!   - Publish new version to crates.io
//!
//! [`cargo-make`]: https://github.com/sagiegurari/cargo-make/
//! [`cargo-readme`]: https://github.com/livioribeiro/cargo-readme
//! [`standard-version`]: https://www.npmjs.com/package/standard-version
//! [conventional commits]: https://www.conventionalcommits.org/en/v1.0.0/

#![deny(missing_docs, missing_doc_code_examples)]
#![cfg_attr(not(test),
            forbid(missing_copy_implementations,
                   missing_debug_implementations,
                   unreachable_pub,
                   unsafe_code,
                   unused_crate_dependencies))]

use std::env;

use warp::Filter;

/// Entry point
#[tokio::main]
pub async fn main() {
  init_logger();

  let api = filters::api().with(warp::log("mergebot"));

  warp::serve(api).run(([127, 0, 0, 1], 3030)).await;
}

/// Warp filters
pub mod filters {
  use warp::Filter;

  /// expands to gross filter type
  macro_rules! filter {() => {impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone}}

  /// The composite warp filter that defines our HTTP api
  pub fn api(
    )
      -> filter!()
  {
    hello().or(deploy())
  }

  /// GET api/v1/hello/:name -> 200 "hello, {name}!"
  fn hello() -> filter!() {
    warp::path!("api" / "v1" / "hello" / String)
         .and(warp::get())
         .map(|name| format!("hello, {}!", name))
  }

  /// Initiate a deployment
  fn deploy() -> filter!() {
    warp::path!("api" / "v1" / "deploy")
         .and(warp::post())
         .and(warp::body::json::<serde_json::Value>())
         .map(|j| Ok(warp::reply::json(&j)))
  }
}

fn init_logger() {
  if env::var_os("RUST_LOG").is_none() {
    env::set_var("RUST_LOG", "mergebot=debug");
  }

  pretty_env_logger::init();
}
