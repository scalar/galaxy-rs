# Changelog

## [0.2.0](https://github.com/scalar/galaxy-rs/compare/v0.1.0...v0.2.0) (2026-08-07)


### Features

* **api:** initial SDK generation ([ac64604](https://github.com/scalar/galaxy-rs/commit/ac646048d6620c065e5375033a5ae880ee159128))


### Chores

* **api:** regenerate SDK ([dc97a87](https://github.com/scalar/galaxy-rs/commit/dc97a878dbac94cf85c94136dce67239a37ec252))
* **api:** regenerate SDK ([2cabe5d](https://github.com/scalar/galaxy-rs/commit/2cabe5def3dee10def50a51072c026f49d01fb0a))

## Changelog

All notable changes to `scalar-galaxy` are documented here. Release
tooling appends a section per released version below.

## Unreleased

- Initial generation of the `scalar-galaxy` SDK.
- Response-only models are marked `#[non_exhaustive]`, so new response
  fields can be added in future versions without a breaking release;
  request models stay literally constructible.
