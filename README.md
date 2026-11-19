# X星图

把一个 X 账号的公开互动圈画成星图。面积是互动强度，颜色是净流入或这一段时间的互动变化。

## 直接运行发布包

从 [Releases](https://github.com/NetSalmon/x-stock-hot-map/releases) 下载对应系统的压缩包，解开后在该目录里运行程序：

- Linux / macOS：`./x-stock-hot-map`
- Windows：`x-stock-hot-map.exe`

浏览器打开 http://127.0.0.1:8787 。页面文件在旁边的 `static/`，圈子数据写在同目录的 `data/`。端口用环境变量 `PORT` 修改。

| 压缩包 | 系统 |
| --- | --- |
| `x-stock-hot-map-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 |
| `x-stock-hot-map-aarch64-unknown-linux-gnu.tar.gz` | Linux arm64 |
| `x-stock-hot-map-x86_64-pc-windows-msvc.tar.gz` | Windows x64 |
| `x-stock-hot-map-aarch64-apple-darwin.tar.gz` | macOS Apple 芯片 |
| `x-stock-hot-map-x86_64-apple-darwin.tar.gz` | macOS Intel |

## 从源码运行

需要 Rust 1.85 或更新版本。

```bash
cargo run --release
```
