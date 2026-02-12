# Changelog

## [v0.3.0] - 2026-02-12

### Features
- add npm binary distribution for `npx code-guardrails` (5eb18de)

### Bug Fixes
- include Cargo.lock in release commit step (de19f11)

## [v0.2.0] - 2026-02-12

### Features
- add /release skill for automated crate publishing (13e9b3e)

### Performance
- parallelize file processing with rayon and reduce redundant work (1c83906)
