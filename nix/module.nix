## kero NixOS 模块（方案A：编排现有原生服务）
##
## 该模块把现有手写的 easytier.nix + syncthing.nix 逻辑收敛成一个可复用模块，
## 生成与之**完全一致**的 systemd 服务与 syncthing 声明式配置（dry-build 应无变化），
## 并额外提供 kero CLI 以及可选的 kero REST 守护进程（kero serve）。
##
## 用法（在主机配置里）：
##   imports = [ inputs.kero.nixosModules.kero ];
##   services.kero = {
##     enable = true;
##     mesh.envFile = config.age.secrets.easytier-env.path;
##     mesh.nodeIps = { angeles-server = "10.144.144.1"; ... };
##     sync = { enable = true; username = "kael"; devices = { ... }; };
##   };
self:
{
  config,
  pkgs,
  lib,
  ...
}:
let
  cfg = config.services.kero;
  host = cfg.host;
  keroPkg = cfg.package;

  # ---- EasyTier（镜像 easytier.nix） ----
  etExec = lib.concatStringsSep " " (
    [
      (lib.getExe' pkgs.easytier "easytier-core")
      "--ipv4 ${cfg.mesh.nodeIps.${host}}"
      "--hostname ${host}"
      "--dev-name ${cfg.mesh.devName}"
    ]
    ++ lib.concatMap (r: [
      "-p tcp://${r}"
      "-p udp://${r}"
    ]) cfg.mesh.relays
  );

  # ---- Syncthing（镜像 syncthing.nix） ----
  # 对端 = 除本机(host)外、且已填好真实 id（非 PLACEHOLDER）的设备。
  peers = lib.filterAttrs (
    name: d: name != host && !(lib.hasSuffix "PLACEHOLDER" d.id)
  ) cfg.sync.devices;

  peerDevices = lib.mapAttrs (name: d: {
    inherit (d) id;
    addresses = [ "tcp://${d.etIp}:22000" ];
  }) peers;
in
{
  options.services.kero = {
    enable = lib.mkEnableOption "kero：统一管理 EasyTier 组网与 Syncthing 同步";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.kero;
      description = "kero 包。";
    };

    host = lib.mkOption {
      type = lib.types.str;
      default = config.networking.hostName;
      description = "本机在 mesh/同步里的标识（需等于 nodeIps / devices 的 key）。";
    };

    restApi = {
      enable = lib.mkEnableOption "运行 kero REST 守护进程（kero serve）";
      address = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1:8391";
        description = "kero REST API 监听地址。";
      };
    };

    mesh = {
      nodeIps = lib.mkOption {
        type = lib.types.attrsOf lib.types.str;
        description = "host => 虚拟网 IPv4。本机 host 必须在表中。";
      };
      relays = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ "70.39.198.217:11010" ];
        description = "公网中继 host:port 列表（tcp+udp 均连接）。";
      };
      devName = lib.mkOption {
        type = lib.types.str;
        default = "et0";
        description = "固定 TUN 接口名。";
      };
      envFile = lib.mkOption {
        type = lib.types.path;
        description = "EnvironmentFile：提供 ET_NETWORK_NAME / ET_NETWORK_SECRET。";
      };
      firewallPorts = lib.mkOption {
        type = lib.types.listOf lib.types.port;
        default = [
          11010
          11011
        ];
        description = "放行的 P2P 打洞/中继回连端口（TCP+UDP）。";
      };
    };

    sync = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "是否在本机启用 Syncthing（easytier-only 主机设为 false）。";
      };
      username = lib.mkOption {
        type = lib.types.str;
        default = "kael";
        description = "运行 syncthing 的用户。";
      };
      guiAddress = lib.mkOption {
        type = lib.types.str;
        default = "0.0.0.0:8384";
        description = "Syncthing Web UI 地址。";
      };
      relaysEnabled = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "是否保留官方中继作兜底（全走虚拟网可设 false）。";
      };
      devices = lib.mkOption {
        type = lib.types.attrsOf (
          lib.types.submodule {
            options = {
              id = lib.mkOption { type = lib.types.str; };
              etIp = lib.mkOption { type = lib.types.str; };
            };
          }
        );
        default = { };
        description = "所有参与同步的设备：key => { id, etIp }。";
      };
      syncDir = lib.mkOption {
        type = lib.types.str;
        default = "/home/${cfg.sync.username}/Sync";
        description = "同步目录。";
      };
      versioningDays = lib.mkOption {
        type = lib.types.str;
        default = "30";
        description = "trashcan 版本保留天数。";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.mesh.nodeIps ? ${host};
        message = "services.kero: host '${host}' 不在 mesh.nodeIps 中。";
      }
    ];

    # ---- EasyTier systemd 服务（与 easytier.nix 一致） ----
    systemd.services.easytier = {
      description = "EasyTier decentralized VPN";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "simple";
        EnvironmentFile = cfg.mesh.envFile;
        ExecStart = etExec;
        Restart = "always";
        RestartSec = 5;
      };
    };

    # ---- Syncthing（与 syncthing.nix 一致），easytier-only 主机可关 ----
    services.syncthing = lib.mkIf cfg.sync.enable {
      enable = true;
      user = cfg.sync.username;
      group = "users";
      dataDir = "/home/${cfg.sync.username}/Sync";
      configDir = "/home/${cfg.sync.username}/.config/syncthing";
      guiAddress = cfg.sync.guiAddress;
      overrideDevices = true;
      overrideFolders = true;
      settings = {
        options = {
          urAccepted = -1;
          crashReportingEnabled = false;
          relaysEnabled = cfg.sync.relaysEnabled;
        };
        devices = peerDevices;
        folders = {
          "default" = {
            label = "Sync";
            path = cfg.sync.syncDir;
            type = "sendreceive";
            devices = lib.attrNames peerDevices;
            versioning = {
              type = "trashcan";
              params.cleanoutDays = cfg.sync.versioningDays;
            };
          };
        };
      };
    };

    # ---- 防火墙（合并 easytier + syncthing 规则） ----
    networking.firewall = {
      trustedInterfaces = [ cfg.mesh.devName ];
      allowedUDPPorts =
        cfg.mesh.firewallPorts
        ++ lib.optionals cfg.sync.enable [
          22000
          21027
        ];
      allowedTCPPorts =
        cfg.mesh.firewallPorts
        ++ lib.optionals cfg.sync.enable [
          8384
          22000
        ];
    };

    # ---- kero REST 守护进程（可选） ----
    systemd.services.kero-serve = lib.mkIf cfg.restApi.enable {
      description = "kero REST API daemon";
      after = [
        "easytier.service"
      ] ++ lib.optional cfg.sync.enable "syncthing.service";
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${lib.getExe keroPkg} serve";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    # kero + easytier-cli 供日常排查。
    environment.systemPackages = [
      keroPkg
      pkgs.easytier
    ];
  };
}
