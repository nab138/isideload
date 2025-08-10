# isideload

A Rust library for sideloading iOS applications. Designed for use in [YCode](https://github.com/nab138/YCode).

## Disclaimer

This package uses private Apple Developer APIs. Use at your own risk.

## Usage

To use isideload, add the following to your `Cargo.toml`:

```toml
[dependencies]
isideload = { version = "0.1.0", features = ["vendored-openssl", "vendored-botan" ] } # Optionally, both vendored features can be enabled to avoid needing OpenSSL and Botan installed on your system.
```

Then, in your Rust code, you can use it as follows:

## Licensing

This project is licensed under the MPL-2.0 License. See the [LICENSE](LICENSE) file for details.

## Credits

- The amazing [idevice](https://github.com/jkcoxson/idevice) crate is used to communicate with the device

- Packages from [`apple-private-apis`](https://github.com/SideStore/apple-private-apis) were used for authentication, but the original project was left unfinished. To support isideload, `apple-private-apis` was forked and modified to add missing features. With permission from the original developers, the fork was published to crates.io until the official project is published.

- [ZSign](https://github.com/zhlynn/zsign) was used for code signing with [custom rust bindings](https://github.com/nab138/zsign-rust)

- [Sideloader](https://github.com/Dadoum/Sideloader) was used as a reference for how the private API endpoints work
