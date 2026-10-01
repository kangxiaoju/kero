# kero

统一管理 **EasyTier（去中心化组网）** 与 **Syncthing（去中心化文件同步）** 的单一 Rust CLI。

一条命令安装两个官方静态二进制，一套 `kero.toml` 配置，统一的进程托管与 REST API 管理接口。
kero 本身不重写同步/组网引擎，而是作为**薄封装编排层**托管官方二进制；同步能力通过
`SyncBackend` trait 抽象，默认实现为 Syncthing，未来可替换为自研 Rust 后端。

详见 [DESIGN.md](./DESIGN.md)。

## 特性

- `kero install` 一次性从 GitHub Release 下载安装 EasyTier + Syncthing（支持代理）
- 关闭两者自带的 web/发现/中继，组网与同步流量全走 EasyTier 隧道，纯去中心化
- 统一配置 `kero.toml`：`[network]` → EasyTier，`[sync]` → Syncthing，`[daemon]` → kero REST API
- 以 REST API 为主管理接口（`kero serve`），TUI 为辅（`kero tui`）
- 支持 easytier-only 主机（省略 `[sync]` + `kero install --no-sync`）

## 快速开始

```bash
kero install [--proxy http://127.0.0.1:7890] [--no-sync]
kero init                 # 生成 kero.toml + syncthing 身份
# 编辑 ~/.kero/kero.toml 填入组网密码、虚拟 IP、peers、对端设备
kero up                   # 先起 easytier，再起 syncthing
kero status               # 查看组网节点 + 同步进度
```

## 常用命令

```
kero install [--proxy URL] [--et-version V] [--st-version V] [--no-sync]
kero init [--force]
kero up [--no-net]        # --no-net：复用已有 mesh，仅起同步
kero down
kero status
kero net peers | info
kero sync add <path> [--id ID] [--device ID...]
kero sync ls | rm <id>
kero device add <id> <addr> [--name N]
kero device id
kero logs [net|sync]
kero upgrade [--check]
kero serve                # REST API 守护进程（默认 127.0.0.1:8391）
kero tui                  # 可选 TUI
```

## REST API

`kero serve` 暴露本地管理接口（默认 `127.0.0.1:8391`）：

```
GET    /health
GET    /status                 net（组网）+ sync（同步进度）
GET    /net/peers | /net/info
GET    /sync/folders
POST   /sync/folders           {path, id?, devices?[]}
DELETE /sync/folders/<id>
GET    /sync/devices
POST   /sync/devices           {id, address, name?}
GET    /sync/id
```

## 构建

```bash
cargo build --release
# 静态二进制（跨机部署）：
cargo build --release --target x86_64-unknown-linux-musl
```

## 许可

MIT
