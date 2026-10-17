# Supplemental upstream licenses

Some crates omit a repository-root license from their crates.io archive.
These files preserve the license from the exact Git commit recorded in that
crate's `.cargo_vcs_info.json`. `SOURCES.json` records each source URL.

htmlescape 0.3.1 has no standalone license file or VCS commit metadata in its
crate. Its published manifest declares `Apache-2.0 / MIT / MPL-2.0`. This package
uses the Apache-2.0 option, includes the standard Apache-2.0 text, and preserves
the upstream 0.3.1 manifest and author attribution. No license or copyright
holder has been invented for it.

These notices do not relicense upstream code. Binary packages additionally
collect crate-local notices (including nested native/Unicode notices) and the
Rust standard-library copyright and license materials.
