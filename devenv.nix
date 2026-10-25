{ ... }:
{
  imports = [
    ./nix/tools.nix
  ];
  # Husky 是唯一提交检查入口；不配置 devenv git hooks。
  enterShell = ''
    echo "ProxyPanel dev shell: Rust backend + Bun management web"
  '';
}
