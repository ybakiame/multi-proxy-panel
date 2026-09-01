import { Alert } from "@heroui/react";
import { useSettingsConfig } from "./useSettingsConfig";
import NetworkSettings from "./NetworkSettings";
import GithubSettings from "./GithubSettings";
import ClashPanelSettings from "./ClashPanelSettings";
import CoreManagement from "./CoreManagement";
import AboutSection from "./AboutSection";
import NotificationSettings from "./NotificationSettings";

export default function Settings() {
  const settings = useSettingsConfig();
  const { error, isAndroid } = settings;

  return (
    <div className="flex max-w-xl flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端连接与核心运行配置</p>
      </div>

      <div className="flex items-center gap-3">
        <span className="text-xs text-muted">所有修改即时保存</span>
      </div>

      <NetworkSettings settings={settings} />
      <GithubSettings settings={settings} />
      <ClashPanelSettings settings={settings} />

      {/* 核心管理：Android 核心为内置 libbox，无「选择核心二进制 / 下载 / 删除」概念，整卡隐藏。 */}
      {!isAndroid && <CoreManagement settings={settings} />}

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

      {/* Notification settings (Android only) */}
      {isAndroid && <NotificationSettings settings={settings} />}
    </div>
  );
}
