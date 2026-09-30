import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.util.Properties

plugins {
  id("com.android.application")
  id("org.jetbrains.kotlin.android")
  id("rust")
}

val tauriProperties =
  Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
      propFile.inputStream().use { load(it) }
    }
  }

android {
  compileSdk = 36
  namespace = "com.proxypanel.client"
  defaultConfig {
    manifestPlaceholders["usesCleartextTraffic"] = "false"
    applicationId = "com.proxypanel.client"
    // minSdk 33（Android 13）：TUN/VpnService 精确路由 + excludeRoute、通知运行时权限等能力的最低要求。
    // 注意：本文件由 `tauri android init` 生成，重新生成时 minSdk 会回落到
    // tauri.conf.json `bundle.android.minSdkVersion` 的值（已同步设置为 33）。
    minSdk = 33
    targetSdk = 36
    versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
    versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    // ABI 裁剪：仅保留 arm64 真机，控制包体（Rust 侧也只构建 aarch64，
    // 见 package.json 的 android:build；x86_64 模拟器如需恢复要连同这里一起改回）。
    ndk {
      abiFilters += listOf("arm64-v8a")
    }
  }
  buildTypes {
    getByName("debug") {
      manifestPlaceholders["usesCleartextTraffic"] = "true"
      isDebuggable = true
      isJniDebuggable = true
      isMinifyEnabled = false
      packaging {
        // ABI 裁剪后仅剩 arm64-v8a，原其余 ABI 的「保留符号」
        // 规则已无对应 ABI；保留符号语义延续到主 ABI arm64-v8a。
        jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
      }
    }
    getByName("release") {
      isMinifyEnabled = true
      proguardFiles(
        *fileTree(".") { include("**/*.pro") }
          .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
          .toList()
          .toTypedArray(),
      )
    }
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_1_8
    targetCompatibility = JavaVersion.VERSION_1_8
  }
  buildFeatures {
    buildConfig = true
  }
  // AGP 9.3.1 的 lintVitalAnalyzeUniversalRelease 在 lint 分析构建脚本时崩溃
  // （Kotlin FIR 报错 `findFirCompiledSymbol only works on compiled declarations`，
  // AGP lint 自身 bug）。release 打包不依赖 lint 结果，关闭 release lint vital。
  lint {
    checkReleaseBuilds = false
  }
}

// AGP 9 移除了 android.kotlinOptions 旧 DSL，改用 KGP 的 compilerOptions
// （对齐 tauri 上游模板）。
kotlin {
  compilerOptions {
    jvmTarget = JvmTarget.JVM_1_8
  }
}

rust {
  rootDirRel = "../../../"
}

dependencies {
  implementation("androidx.webkit:webkit:1.14.0")
  implementation("androidx.appcompat:appcompat:1.7.1")
  implementation("androidx.core:core-ktx:1.13.1")
  implementation("androidx.activity:activity-ktx:1.10.1")
  implementation("com.google.android.material:material:1.12.0")
  implementation("androidx.lifecycle:lifecycle-process:2.10.0")
  // panelcore Android 库（sing-box libbox 的 gomobile 绑定），由
  // apps/mobile/scripts/build-panel-core.sh 构建，本地构建产物，不入库。
  implementation(files("libs/panelcore.aar"))
  testImplementation("junit:junit:4.13.2")
  androidTestImplementation("androidx.test.ext:junit:1.1.4")
  androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")