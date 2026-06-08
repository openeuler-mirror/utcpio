# utcpio

English (./README.md)

utcpio is a fundamental command-line project that rewrites core Linux commands in the Rust programming language. It is designed for both server and desktop environments and leverages Rust's safety features to provide more secure fundamental operating system commands.

-----

## Environment Requirements

Rust (`cargo`, `rustc`) \>= 1.82.0

-----

## How to Build

We use Cargo to build the utcpio binary.

First, you need to clone the repository:

```shell
https://gerrit-dev.uniontech.com/admin/repos/V25/utcpio
cd utcpio
```

Then, you can use Cargo to build utcpio, which is the same process as with any other Rust program:

```shell
cargo build --release
```

This command builds utcpio into a binary file named "utcpio."

-----

## How to Install

Use Cargo to install utcpio:

```shell
cargo install --path . --locked
```

This command installs utcpio to Cargo's **bin** folder (e.g., `$HOME/.cargo/bin`). After installation, you can use utcpio with `$HOME/.cargo/bin/utcpio [util] [util options]`.

-----

## How to Uninstall

Use Cargo to uninstall utcpio:

```shell
cargo uninstall
```


-----

## LICENSE

utcpio is licensed under the GPL-3.0-or-later license. For more details, please see the [LICENSE](https://www.google.com/search?q=LICENSE) file.