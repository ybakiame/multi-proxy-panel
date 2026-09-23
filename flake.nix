{
  description = "ProxyPanel 开发环境：Rust(mobile/desktop/后端) + Android(NDK/SDK) + Go(panelcore) + 前端(Bun)";

  nixConfig = {
    extra-substituters = [
      "https://mirrors.ustc.edu.cn/nix-channels/store"
      "https://cache.nixos.org"
    ];
    extra-trusted-public-keys = [
      "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    android-nixpkgs.url = "github:tadfisher/android-nixpkgs/stable";
  };

  outputs = { self, nixpkgs, android-nixpkgs }:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};

    # 声明式 Android SDK：版本与 CI/真机验证所用完全一致
    android-sdk = android-nixpkgs.sdk.${system} (sdkPkgs: with sdkPkgs; [
      cmdline-tools-latest
      platform-tools
      platforms-android-35
      build-tools-35-0-0
      ndk-28-0-13004108
    ]);
    ndkVersion = "28.0.13004108";
  in {
    devShells.${system}.default = pkgs.mkShell {
      # 构建基础与工具
      packages = with pkgs; [
        git
        curl
        wget
        unzip
        gnumake
        cmake
        ninja
        pkg-config
        perl # openssl-sys(vendored) 构建
        protobuf # pp-proto / tonic 代码生成

        # Rust 生态（工具链本体由 rustup 按 rust-toolchain.toml 锁定）
        rustup
        cargo-ndk # Android 交叉编译 + jniLibs 打包

        # Android（声明式 SDK/NDK 见 android-sdk；AGP / gomobile / adb）
        android-sdk
        jdk17
        gradle

        # Go（panelcore / gomobile）
        go

        # 前端（Bun workspaces）
        bun
        nodejs

        # 桌面 GUI 系统库（Tauri / Dioxus desktop / Slint host）
        webkitgtk_4_1
        gtk3
        glib
        glib-networking # webkit 运行时 TLS
        gdk-pixbuf
        pango
        cairo
        libsoup_3
        openssl
        alsa-lib
        libxkbcommon
        libX11
        libXcursor
        libXrandr
        libXi
      ];

      env = {
        # Gradle / gomobile 统一使用 flake 提供的 JDK
        JAVA_HOME = "${pkgs.jdk17.home}";
        # 声明式 Android SDK/NDK（tadfisher/android-nixpkgs，许可证已预接受）
        # android-nixpkgs 组合 SDK 安装于 share/android-sdk 子目录
        ANDROID_HOME = "${android-sdk}/share/android-sdk";
        ANDROID_SDK_ROOT = "${android-sdk}/share/android-sdk";
        ANDROID_NDK_ROOT = "${android-sdk}/share/android-sdk/ndk/${ndkVersion}";
        ANDROID_JAR = "${android-sdk}/share/android-sdk/platforms/android-35/android.jar";
      };

      shellHook = ''
        # 让 nixpkgs 的 pkg-config 优先发现 flake 提供的系统库
        export PKG_CONFIG_PATH="${pkgs.webkitgtk_4_1.dev}/lib/pkgconfig:${pkgs.gtk3.dev}/lib/pkgconfig:${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.glib.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"

        echo "ProxyPanel dev shell: go=$(go version 2>/dev/null | cut -d' ' -f3) bun=$(bun --version 2>/dev/null) ndk=$(basename "$ANDROID_NDK_ROOT")"
      '';
    };
  };
}
