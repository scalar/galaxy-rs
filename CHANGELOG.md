# Changelog

## [0.3.1](https://github.com/scalar/galaxy-rs/compare/v0.3.0...v0.3.1) (2026-08-31)


### Chores

* **api:** regenerate SDK ([35e3105](https://github.com/scalar/galaxy-rs/commit/35e31056241773da5fb5ea5b0cc76144fdc75c5b))
* **api:** update generated SDK content ([9357600](https://github.com/scalar/galaxy-rs/commit/9357600112c3b621ea54d2c4eedd3fbd1dcdd9a6))

## [0.3.0](https://github.com/scalar/galaxy-rs/compare/v0.2.0...v0.3.0) (2026-08-28)


### ⚠ BREAKING CHANGES

* **api:** Removed environment `responds_with_your_request_data`.
* **api:** 3 breaking changes to the SDK surface.
    - Removed operation `planets.uploadImage` (`POST /planets/{planetId}/image`).
    - Removed schema `UploadImageResponseHeaders`.
    - Removed schema `UploadImageStatus400ResponseHeaders`.

### Features

* **api:** remove operation planets.uploadImage (+6 more changes) ([61e3b6e](https://github.com/scalar/galaxy-rs/commit/61e3b6edc7e7b98d0887dc00b90282675c293c55))
* **api:** update SDK surface (2 changes) ([51d1d79](https://github.com/scalar/galaxy-rs/commit/51d1d79b7e36b99182b3440aa2645f6c09708202))


### Chores

* **api:** regenerate SDK ([84aaf5b](https://github.com/scalar/galaxy-rs/commit/84aaf5b6ea702b69067fecdc95bd137a7052a27c))
* **api:** regenerate SDK ([dcaf07c](https://github.com/scalar/galaxy-rs/commit/dcaf07cccce74bd44d7a2e911ae59c21991b73f0))
* **api:** regenerate SDK ([1cf0aab](https://github.com/scalar/galaxy-rs/commit/1cf0aab68aba644f3d4a19b80a440fbc1257e123))
* **api:** update generated SDK content ([05b00d5](https://github.com/scalar/galaxy-rs/commit/05b00d574fa9d773aec7f4216197f63132021ff5))
* **api:** update generated SDK content ([0c53e75](https://github.com/scalar/galaxy-rs/commit/0c53e75e80c0c43cb83ce796ca1fd7dca3f4dce9))
* **api:** update generated SDK content ([b9497a6](https://github.com/scalar/galaxy-rs/commit/b9497a6f3af1fc9c70ac2ed3c209f83874dc767c))

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
