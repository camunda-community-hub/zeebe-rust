# Changelog

## [v0.5.0](https://github.com/fathom-io/zeebe-rust/compare/v0.4.2...v0.5.0)

### Changed

- Client is now Send + Sync
- Updated to new zeebe protocol

## [v0.4.2](https://github.com/OutThereLabs/zeebe-rust/compare/v0.4.1...v0.4.2)

### Fixed

- Error events in `tracing` logs

## [v0.4.1](https://github.com/OutThereLabs/zeebe-rust/compare/v0.4.0...v0.4.1)

### Fixed

- Add oauth request timeouts (#30)

## [v0.4.0](https://github.com/OutThereLabs/zeebe-rust/compare/v0.3.1...v0.4.0)

### Added

- Add OAuth Support (#28)

### Changed

- Update to zeebe 1.0 protocol (#27)

## [v0.3.1](https://github.com/OutThereLabs/zeebe-rust/compare/v0.3.0...v0.3.1)

### Fixed

- Allow dispatcher to properly process concurrent jobs (#14)

## [v0.3.0](https://github.com/OutThereLabs/zeebe-rust/compare/v0.2.1...v0.3.0)

### Changed

- Update to tonic 0.4 and tokio 1.0 #12

## [v0.2.1](https://github.com/OutThereLabs/zeebe-rust/compare/v0.2.0...v0.2.1)

### Changed

- Update default request and long polling timeouts (#4)
- Add tracing instrumentation to auto handlers (#5)

## [v0.2.0](https://github.com/OutThereLabs/zeebe-rust/compare/v0.1.2...v0.2.0)

### Changed

- Switch to extractors for auto handlers #3

  To upgrade current auto handlers, wrap parameters in `Data<T>`:

  ```diff
  - .with_auto_handler(|client: Client, job_data: MyJobData| ..)
  + .with_auto_handler(|client: Client, job_data: zeebe::Data<MyJobData>| ..)
  ```

## [v0.1.2](https://github.com/OutThereLabs/zeebe-rust/compare/v0.1.1...v0.1.2)

### Changed

- Removed need for handler function future results to be `Send` + `Sync` #2

## [v0.1.1](https://github.com/OutThereLabs/zeebe-rust/compare/v0.1.0...v0.1.1)

### Added

- Simplify common tasks by assuming current job in methods that accept
  `with_job_key` #1

## [v0.1.0](https://github.com/OutThereLabs/zeebe-rust/tree/v0.1.0)

Initial release
