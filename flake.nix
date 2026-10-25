{
  description = "ProxyPanel 开发环境：Rust 服务端 + Bun 管理 Web";

  nixConfig = {
    substituters = [
      "https://mirrors.ustc.edu.cn/nix-channels/store?priority=10"
      "https://mirrors.tuna.tsinghua.edu.cn/nix-channels/store?priority=15"
      "https://mirror.sjtu.edu.cn/nix-channels/store?priority=20"
      "https://cache.nixos.org"
    ];
    extra-substituters = [
      "https://mirrors.cernet.edu.cn/nix-channels/store"
    ];
    extra-trusted-public-keys = [
      "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
    ];
  };

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          runtimeLibs = with pkgs; [ zlib xz bzip2 zstd ];
        in {
          default = pkgs.mkShell {
            packages = runtimeLibs ++ (with pkgs; [
              git curl wget unzip gnumake cmake ninja pkg-config perl protobuf
              rustup bun nodejs
            ]);
            shellHook = ''
              export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath runtimeLibs}:''${LD_LIBRARY_PATH:-}"
              echo "ProxyPanel dev shell: Rust backend + Bun management web"
            '';
          };
        });
    };
}
