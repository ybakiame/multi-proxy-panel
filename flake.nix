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
      # 35: gomobile / ANDROID_JAR 等仍在使用；36: tauri 2.11 生成的工程
      # compileSdk/targetSdk = 36，AGP 默认 build-tools 36.0.0
      platforms-android-35
      platforms-android-36
      build-tools-35-0-0
      build-tools-36-0-0
      ndk-28-0-13004108
    ]);
    ndkVersion = "28.0.13004108";
    ndkHome = "${android-sdk}/share/android-sdk/ndk/${ndkVersion}";
    ndkLlvm = "${ndkHome}/toolchains/llvm/prebuilt/linux-x86_64";
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
        ANDROID_NDK_ROOT = ndkHome;
        ANDROID_JAR = "${android-sdk}/share/android-sdk/platforms/android-35/android.jar";
        # tauri CLI 探测 NDK 时优先读 NDK_HOME（其次才扫 $ANDROID_HOME/ndk），
        # 必须显式设置，否则用户 shell rc 里的主机 NDK 会污染构建
        NDK_HOME = ndkHome;
        ANDROID_NDK_HOME = ndkHome;
        # 裸 cargo 交叉编译（cargo check/build --target aarch64-linux-android）同样走
        # nix NDK。mobile 仅发布 arm64（abiFilters），故只导出 aarch64 一套；
        # tauri android build 会按 NDK_HOME 再注入自己的工具链 env 覆盖此处，两者一致。
        CC_aarch64_linux_android = "${ndkLlvm}/bin/aarch64-linux-android33-clang";
        AR_aarch64_linux_android = "${ndkLlvm}/bin/llvm-ar";
        CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "${ndkLlvm}/bin/aarch64-linux-android33-clang";
        # rquickjs-sys Android target 的 bindgen 需要 NDK sysroot
        BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android = "--target=aarch64-linux-android33 --sysroot=${ndkLlvm}/sysroot -isystem ${ndkLlvm}/sysroot/usr/include";
      };

      shellHook = ''
        # 让 nixpkgs 的 pkg-config 优先发现 flake 提供的系统库
        export PKG_CONFIG_PATH="${pkgs.webkitgtk_4_1.dev}/lib/pkgconfig:${pkgs.gtk3.dev}/lib/pkgconfig:${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.glib.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"

        echo "ProxyPanel dev shell: go=$(go version 2>/dev/null | cut -d' ' -f3) bun=$(bun --version 2>/dev/null) ndk=$(basename "$ANDROID_NDK_ROOT")"
      '';
    };
  };
}
