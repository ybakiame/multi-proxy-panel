{ pkgs, ... }:
let
  # Agent 压缩库（sing-box/mihomo 依赖），运行时库路径由 enterShell 前置注入。
  runtimeLibs = with pkgs; [ zlib xz bzip2 zstd ];
in
{
  packages = runtimeLibs ++ (with pkgs; [
    git curl wget unzip gnumake cmake ninja pkg-config perl protobuf
    rustup bun nodejs
  ]);

  enterShell = ''
    export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath runtimeLibs}:''${LD_LIBRARY_PATH:-}"
  '';
}
