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

## 安装

### 方式一：下载预编译二进制（推荐，非 NixOS）

从 [Releases](https://github.com/kangxiaoju/kero/releases) 下载对应平台的包：

| 平台 | 文件 |
|---|---|
| Linux x86_64 | `kero-x86_64-unknown-linux-musl.tar.gz`（静态，无依赖） |
| Linux aarch64 | `kero-aarch64-unknown-linux-musl.tar.gz` |
| macOS Intel | `kero-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `kero-aarch64-apple-darwin.tar.gz` |

```bash
tar xzf kero-x86_64-unknown-linux-musl.tar.gz
sudo install kero /usr/local/bin/
```

### 方式二：cargo 安装

```bash
cargo install --git https://github.com/kangxiaoju/kero
```

### 方式三：NixOS（声明式，本仓库自带模块）

在你的 flake 里引入，用模块声明组网与同步，无需 `kero init`/`kero up`：

```nix
# flake.nix inputs
kero.url = "github:kangxiaoju/kero";

# 配置中
imports = [ inputs.kero.nixosModules.kero ];
services.kero = {
  enable = true;
  mesh = { envFile = config.age.secrets.easytier-env.path; nodeIps = { … }; relays = [ … ]; };
  sync = { enable = true; inherit username; devices = { … }; };
};
```

> NixOS 下 kero 直接生成 systemd 服务与 syncthing 声明式配置，运行时**不使用** `~/.kero/kero.toml`；该文件仅用于非 NixOS 的 `kero init` 手工部署。

## 快速开始（非 NixOS）

```bash
kero install [--proxy http://127.0.0.1:7890] [--no-sync]
kero init                 # 生成 ~/.kero/kero.toml + syncthing 身份
# 编辑 ~/.kero/kero.toml 填入组网密码、虚拟 IP、peers、对端设备
kero up                   # 先起 easytier，再起 syncthing
kero status               # 查看组网节点 + 同步进度
```

### 新设备接入已有 mesh

```bash
kero install --proxy http://127.0.0.1:7890
kero init
# kero.toml: [network] name/secret 填 mesh 凭据，ip 选未用虚拟 IP，
#            peers 填中继，如 ["tcp://<relay>:11020","udp://<relay>:11020"]
kero up
kero device id                                    # 取本机 syncthing id
kero device add <对端id> tcp://<对端虚拟IP>:22000 --name <名称>   # 登记每个对端
kero sync add ~/Sync --device <对端id>             # 加同步目录并共享
kero status
# 最后：在每个已有节点也把本机 id 加为对端（双向信任才建立同步）
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
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

推送 `v*` tag 会触发 GitHub Actions（`.github/workflows/release.yml`）在原生
Linux/macOS runner 上构建四平台二进制并自动发布到 Releases。

## 许可

MIT
