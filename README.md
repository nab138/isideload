# isideload

[![Build isideload](https://github.com/nab138/isideload/actions/workflows/build.yml/badge.svg)](https://github.com/nab138/isideload/actions/workflows/build.yml)

A Rust library for sideloading iOS applications using an Apple ID. Used in [CrossCode](https://github.com/nab138/CrossCode) and [iloader](https://github.com/nab138/iloader).

This branch is home to isideload-next, the next major version of isideload. It features a redesigned API, improved error handling, better entitlement handling, and more. It is not ready!

## TODO

Things left todo before the rewrite is considered finished

- Download provisioning profiles
- Signing apps
- Installing apps
(will superceed the original isideload at this point)
- Remove dependency on ring
- More parallelism/cachng for better performance

## Licensing

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
