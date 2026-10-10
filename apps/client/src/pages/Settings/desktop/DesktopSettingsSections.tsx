import { Alert } from "@pp/ui";
import { useSettingsConfig } from "./useSettingsConfig";
import GithubSettings from "./GithubSettings";
import CoreManagement from "./CoreManagement";
import AboutSection from "./AboutSection";

/**
 * 设置页。入站（混合端口 / TUN / IPv6）与 Clash 面板均已剥离至配置页
 * （`/config/inbounds`、`/config/experimental`，对齐移动端），本页保留外观 /
 * GitHub 访问 / 核心管理 / 关于。
 */
export default function DesktopSettingsSections() {
  const settings = useSettingsConfig();
  const { error } = settings;

  return (
    <>
      <GithubSettings settings={settings} />

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
    </>
  );
}
