# isideload

[![Build isideload](https://github.com/nab138/isideload/actions/workflows/build.yml/badge.svg)](https://github.com/nab138/isideload/actions/workflows/build.yml)

A Rust library for sideloading iOS applications using an Apple ID. Used in [CrossCode](https://github.com/nab138/CrossCode) and [iloader](https://github.com/nab138/iloader).

This branch is home to isideload-next, the next major version of isideload. It features a redesigned API, improved error handling, better entitlement handling, and more. It is not ready!

## TODO

Things left todo before the rewrite is considered finished

- Proper entitlement handling
  - actually parse macho files and stuff, right now it just uses the bare minimum and applies extra entitlements for livecontainer
- Reduce duplicate dependencies
  - partially just need to wait for the rust crypto ecosystem to get through another release cycle
- More parallelism and caching for better performance

## Licensing

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Credits

- The [idevice](https://github.com/jkcoxson/idevice) crate is used to communicate with the device
- A [modified version of apple-platform-rs](https://github.com/nab138/isideload-apple-platform-rs) was used for codesigning, based off [plume-apple-platform-rs](https://github.com/plumeimpactor/plume-apple-platform-rs)
- [Sideloader](https://github.com/Dadoum/Sideloader) was used as a reference for how some of the private API endpoints work
- [Impactor](https://github.com/khcrysalis/Impactor) was used as a reference for some cryptography code
