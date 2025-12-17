# utcpio

简体中文(./README.zh_CN.md)

utcpio 是一个基础命令行的项目，该项目使用 Rust 语言重写 Linux 下的基础命令，支持服务器场景以及桌面场景。并借助 Rust 的安全能力，提供更为安全的操作系统基础命令。

## 环境要求

Rust (`cargo`, `rustc`) >= 1.82.0

## 构建方法

我们使用 Cargo 来构建 utcpio 二进制文件。

我们首先需要拉取仓库：

```shell
https://gerrit-dev.uniontech.com/admin/repos/V25/utcpio
cd utcpio
```

然后我们可以使用 Cargo 构建 utcpio，该流程与其他 Rust 程序相同：

```shell
cargo build --release
```

此命令将 utcpio 构建为名为 “utcpio” 的二进制文件。

## 安装方法

使用 Cargo 安装 utcpio

```shell
cargo install --path . --locked
```

此命令将 utcpio 安装到 Cargo 的 _bin_ 文件夹中（例如 `$HOME/.cargo/bin`）。之后，可以通过 `$HOME/.cargo/bin/utcpio [util] [util options]` 使用 utcpio。

## 卸载方法

使用 Cargo 卸载 utcpio：

```shell
cargo uninstall 
```

## 参与贡献

参与 ut 贡献，请参阅 [CONTRIBUTING](CONTRIBUTING.md) 文件。

## LICENSE

utcpio 使用 GPL-3.0-or-later 许可证，详细信息请参阅 [LICENSE](LICENSE) 文件。

