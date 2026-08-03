import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
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
        // minSdk 26：TUN/VpnService 与系统通知等能力的最低要求。
        // 注意：本文件由 `tauri android init` 生成，重新生成时 minSdk 会回落到
        // tauri.conf.json `bundle.android.minSdkVersion` 的值（已同步设置为 26）。
        minSdk = 26
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
        // ABI 裁剪：仅保留 arm64 真机 + x86_64 模拟器，控制包体。
        // libbox.aar / 未来 mihomo.aar 内多余 ABI 的 .so 由 abiFilters 在打包时剔除。
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                // ABI 裁剪后仅剩 arm64-v8a / x86_64，原 armeabi-v7a 与 x86 的「保留符号」
                // 规则已无对应 ABI；保留符号语义延续到主 ABI arm64-v8a 与 x86_64。
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
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
    // panelcore（合并核心）Android 库，由 scripts/build-panel-core.sh 合并构建，
    // 同时含 sing-box libbox 与 mihomo 双核心绑定（gomobile 单库单运行时，
    // 避免 libbox.aar + mihomo.aar 双 go.* 运行时冲突），本地构建产物，不入库。
    implementation(files("libs/panelcore.aar"))
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")