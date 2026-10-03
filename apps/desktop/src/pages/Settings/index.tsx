import { Alert } from "@heroui/react";
import { useSettingsConfig } from "./useSettingsConfig";
import AppearanceSettings from "./AppearanceSettings";
import GithubSettings from "./GithubSettings";
import ClashPanelSettings from "./ClashPanelSettings";
import CoreManagement from "./CoreManagement";
import AboutSection from "./AboutSection";

/**
 * 设置页。入站（混合端口 / TUN / IPv6）已剥离至「配置 → 入站管理」
 * （`/config/inbounds`，对齐移动端），本页保留外观 / GitHub 访问 / Clash 面板 /
 * 核心管理 / 关于。
 */
export default function Settings() {
  const settings = useSettingsConfig();
  const { error } = settings;

  return (
    <div className="flex max-w-xl flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端连接与核心运行配置</p>
      </div>

      <div className="flex items-center gap-3">
        <span className="text-xs text-muted">所有修改即时保存</span>
      </div>

      <AppearanceSettings />
      <GithubSettings settings={settings} />
      <ClashPanelSettings settings={settings} />

      <CoreManagement settings={settings} />

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>加载失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      <AboutSection />
    </div>
  );
}
