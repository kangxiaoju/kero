# Kero 设计文档

Kero 是一个用 Rust 编写的单一 CLI，用于把 **EasyTier（组网）** 与 **Syncthing（去中心化同步）**
封装成一个统一工具：一条命令安装、一套配置、统一的进程托管与管理接口（REST API，可选 TUI）。

Kero 本身不重写同步/组网引擎，而是作为**薄封装编排层**托管两个官方静态二进制。
同步能力通过 `SyncBackend` trait 抽象，默认实现为 Syncthing，未来可替换为自研 Rust 后端。

## 1. 目标与非目标

目标：
- 一次 `kero install` 同时安装 EasyTier + Syncthing（从 GitHub Release 下载，支持代理）。
- 一个 CLI 同时管理组网与同步目录。
- 资源占用低、可长期不间断运行。
- 跨平台：优先 Linux 与 macOS。
- 支持被托管二进制的版本升级（`kero upgrade`）。
- 关闭 EasyTier / Syncthing 自带的 web 页面；管理以 kero 的 REST API 为主、TUI 为辅。

非目标：
- 不重写同步/组网核心逻辑（保留 trait 以便将来自研）。
- 不接管用户系统上已存在的 EasyTier / Syncthing 服务。

## 2. 关键隔离原则（不破坏现网）

测试/运行机可能已存在独立的 EasyTier / Syncthing。Kero 必须完全隔离：
- 所有 kero 托管的进程使用**独立数据目录**（`~/.kero/`），绝不读写 `/etc` 或 `~/.config/syncthing`。
- EasyTier 使用**独立网络**：独立 network-name / secret / TUN 设备名（`ket0`）/ 网段（默认 `10.145.145.0/24`）。
- 使用**独立端口**，默认全部偏移，避免与常见默认端口冲突：
  - EasyTier RPC portal: `15899`（官方默认 15888）
  - EasyTier peer listen: `11020`（官方默认 11010）
  - Syncthing sync (BEP): `22010`（官方默认 22000）
  - Syncthing REST/GUI: `127.0.0.1:8390`，仅本地，仅供 kero 调 REST（官方默认 8384）
  - 所有端口可在 kero.toml 覆盖。

## 3. 架构

```
            ┌─────────────────────────── kero (单一 Rust 二进制) ──────────────────────────┐
            │  CLI (clap)   REST API (serve)   TUI (ratatui, 可选)                          │
            │       │            │                                                          │
            │  ┌────▼────────────▼────┐                                                     │
            │  │   编排 / 状态管理     │  config.rs  state.rs  process.rs                    │
            │  └───┬──────────────┬───┘                                                     │
            │   net│模块        sync│模块(SyncBackend trait)                                │
            └──────┼──────────────┼──────────────────────────────────────────────────────┘
                   │ 生成 toml      │ REST API (127.0.0.1)
                   │ + spawn        │ + spawn
             ┌─────▼──────┐   ┌─────▼──────────┐
             │easytier-core│   │   syncthing    │   ← kero 托管的官方二进制（~/.kero/bin）
             └────────────┘   └────────────────┘
```

进程托管采用**方案 A**：kero 直接 spawn 子进程，PID / 日志写入 `~/.kero/run/`，
`kero up` / `kero down` 管理生命周期。（systemd 常驻可作为后续可选项，不在首版范围。）

## 4. 目录布局

### 源码
```
kero/
├── Cargo.toml
├── DESIGN.md
└── src/
    ├── main.rs           # 入口，分发 CLI
    ├── cli.rs            # clap 子命令定义
    ├── config.rs         # kero.toml 解析 (serde)
    ├── paths.rs          # ~/.kero 路径解析
    ├── state.rs          # 运行时状态 (pid/port/version)
    ├── process.rs        # 子进程 spawn / 停止 / 存活检测 (方案A)
    ├── platform.rs       # OS/arch 检测 + release 资产名匹配
    ├── install/
    │   ├── mod.rs
    │   ├── github.rs     # release 查询 + 下载 + sha256 校验 + 代理
    │   └── extract.rs    # zip / tar.gz 解压取出目标二进制
    ├── net/
    │   └── mod.rs        # EasyTier: 生成 toml + 托管 + easytier-cli 查询
    ├── sync/
    │   ├── mod.rs        # SyncBackend trait（抽象，留自研路）
    │   └── syncthing.rs  # Syncthing 后端：spawn(无web) + REST 客户端
    └── tui/
        └── mod.rs        # ratatui 管理界面
```

### 运行时 (`~/.kero/`)
```
~/.kero/
├── kero.toml            # 用户唯一需编辑的配置
├── bin/                 # easytier-core, easytier-cli, syncthing
├── versions.json        # 已安装二进制版本（供 upgrade 比对）
├── easytier/
│   └── config.toml      # kero 生成的 EasyTier 配置
├── syncthing/           # syncthing home (config + index db)
├── run/
│   ├── easytier.pid / .log
│   └── syncthing.pid / .log
└── state.json           # 运行状态缓存
```

## 5. 统一配置 kero.toml

```toml
[network]                        # → EasyTier
name       = "kero-net"          # 独立网络名
secret     = "change-me"
ip         = "10.145.145.1"      # 本机虚拟 IP（也可用 dhcp=true）
dhcp       = false
hostname   = "desktop"           # 可选，默认取系统 hostname
peers      = ["tcp://PUBLIC:11010"]
dev_name   = "ket0"
rpc_portal = "127.0.0.1:15899"
listeners  = ["tcp://0.0.0.0:11020"]

[sync]                           # → Syncthing 全局
listen_ip   = "10.145.145.1"     # BEP 监听绑定到虚拟 IP
sync_port   = 22010
api_addr    = "127.0.0.1:8390"   # 仅本地 REST（无面向用户 web）
# kero 自动关闭 global/local discovery、relay、nat
# [sync] 整段可省略 → easytier-only 主机（配合 `kero install --no-sync`）

[daemon]                         # kero 自身 REST API（kero serve）
api_addr = "127.0.0.1:8391"      # 仅本地，管理 mesh + 同步

[[sync.folder]]                  # → Syncthing folder（经 REST 下发）
id    = "test"
label = "test"
path  = "/home/kael/kero-sync-test"
devices = ["PEER-DEVICE-ID"]

[[sync.device]]                  # → Syncthing device，地址用虚拟 IP
id      = "PEER-DEVICE-ID"
name    = "fedora"
address = "tcp://10.145.145.2:22010"
```

Kero 把 `[network]` 翻译成 `easytier/config.toml`；把 `[sync]` 通过 Syncthing REST API 下发。

## 6. CLI 命令

```
kero install [--proxy URL] [--et-version V] [--st-version V] [--no-sync]
                         # 下载安装 easytier + syncthing 到 ~/.kero/bin
                         # --no-sync：仅装 easytier（easytier-only 主机）
kero init                # 生成默认 kero.toml + syncthing 身份 (generate)
kero up                  # 启动 easytier→syncthing（先网络后同步）
kero down                # 停止两者
kero status              # 合并展示：组网节点 + 同步进度
kero net peers           # easytier 组网节点（转调 easytier-cli）
kero sync add <path> [--id ID] [--device ID...]   # 加同步目录 (REST)
kero sync ls             # 列同步目录及进度 (REST)
kero sync rm <id>        # 删同步目录
kero device add <id> <addr> [--name N]            # 加对端设备 (REST)
kero device id           # 显示本机 syncthing device id
kero logs [net|sync]     # 查看日志
kero upgrade [--check]   # 检查/升级被托管二进制
kero serve               # 运行 REST API 守护进程（127.0.0.1:8391，主管理接口）
kero tui                 # 进入 TUI 管理界面（可选）
```

### REST API（`kero serve`，默认 `127.0.0.1:8391`）

```
GET    /health                 健康检查
GET    /status                 合并状态：net（组网）+ sync（同步进度）
GET    /net/peers              组网节点（转调 easytier-cli）
GET    /net/info               本机节点信息
GET    /sync/folders           列同步目录及进度
POST   /sync/folders           加同步目录 {path, id?, devices?[]}
DELETE /sync/folders/<id>      删同步目录
GET    /sync/devices           列对端设备（Syncthing 原始配置）
POST   /sync/devices           加对端设备 {id, address, name?}
GET    /sync/id                本机 syncthing device id
```

easytier-only 主机（无 `[sync]`）上，`/sync/*` 返回 400。

## 7. install / upgrade 设计

- `platform.rs` 把 `(os, arch)` 映射到资产名：
  - EasyTier: `easytier-{linux-x86_64|linux-aarch64|macos-x86_64|macos-aarch64}-v{ver}.zip`
  - Syncthing: `syncthing-{linux-amd64|linux-arm64|macos-amd64|macos-arm64}-v{ver}.{tar.gz|zip}`
- `github.rs`：查 `releases/latest`（或指定 tag）→ 选资产 → 下载 →（有 sha256sum 时）校验 → 交给 `extract.rs`。
- `extract.rs`：zip / tar.gz 解包，仅取出 `easytier-core`、`easytier-cli`、`syncthing`，`chmod +x`，放入 `~/.kero/bin`。
- 记录到 `versions.json`。
- `upgrade --check` 比对本地与最新 tag；`upgrade` 下载新版，先停服务、替换、再启动。
- 代理：`reqwest` 读取 `--proxy` 或 `HTTPS_PROXY` 环境变量。
- NixOS 兼容：下载的动态二进制经 `nix-ld` 可直接运行；install 结束用 `--version` 自检，失败则明确报错。

## 8. 关闭 web 的具体做法

- EasyTier：不传 `-w/--config-server`，不下载 web dashboard，core 本身无 web。
- Syncthing：
  - 启动用 `--no-browser`、`--no-default-folder`。
  - GUI/REST 监听绑 `127.0.0.1:8390`（仅 kero 内部调 REST，非面向用户页面）。
  - 关闭 `globalAnnounce` / `localAnnounce` / `relays` / `natTraversal`（经 REST 或生成 config）。
- 面向用户的管理以 kero REST API（`kero serve`）为主，TUI 为辅；easytier/syncthing 自身 web 全部关闭。

## 9. SyncBackend 抽象（预留自研路）

```rust
pub trait SyncBackend {
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn device_id(&self) -> Result<String>;
    fn add_device(&self, id: &str, addr: &str, name: &str) -> Result<()>;
    fn add_folder(&self, f: &FolderSpec) -> Result<()>;
    fn remove_folder(&self, id: &str) -> Result<()>;
    fn list_folders(&self) -> Result<Vec<FolderStatus>>;
    fn status(&self) -> Result<SyncStatus>;
}
```

首版仅 `SyncthingBackend`。将来可加 `IrohBackend` 或纯自研后端，CLI/配置不变。

## 10. 资源占用目标

- kero 自身常驻（serve/TUI/编排）内存目标 < 20 MB；非常驻模式下 `up` 后可退出，仅留被托管进程。
- 被托管进程按官方特性运行；kero 通过关闭发现/中继进一步降低连接与内存开销。

## 11. 测试计划

- 本机 desktop (NixOS, 10.144.144.3) 构建并跑通 install。
- 对端 local-fedora-server (10.144.144.2, x86_64, 局域网 2.5ms, 可 SSH) 部署 kero。
- 两端建立独立 kero 网络 + 同步一个全新测试目录，验证双向同步，全程不触碰现网服务。
- 测量两端 kero 托管进程的内存/CPU。
