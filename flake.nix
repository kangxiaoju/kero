{
  description = "kero — 统一管理 EasyTier 组网与 Syncthing 同步的 Rust CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        kero = pkgs.rustPlatform.buildRustPackage {
          pname = "kero";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          # reqwest 使用 rustls-tls，无需 openssl；运行时不依赖这些库，
          # 但 kero 托管的官方二进制在 NixOS 上需 nix-ld（见 nixosModule）。
          nativeBuildInputs = [ pkgs.pkg-config ];

          # 纯编排器，无集成测试。
          doCheck = false;

          meta = with pkgs.lib; {
            description = "Unified CLI to install and manage EasyTier + Syncthing";
            homepage = "https://github.com/kangxiaoju/kero";
            license = licenses.mit;
            mainProgram = "kero";
            platforms = platforms.unix;
          };
        };
        default = kero;
      });

      nixosModules.kero = import ./nix/module.nix self;

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rust-analyzer
            pkgs.clippy
          ];
        };
      });
    };
}
