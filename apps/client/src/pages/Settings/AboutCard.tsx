import { Card } from "@pp/ui";
import { useCoreVersion } from "@pp/client-core";
import { AboutInfo } from "./AboutInfo";
import { SING_BOX_BUILD_TAGS } from "../../lib/coreBuildInfo";

/** 核心版本运行时读取；tags 展示构建配置，产物可用 go version -m 核验。 */
export function AboutCard() {
  const { data: coreVersion, isLoading: versionLoading } = useCoreVersion();

  return (
    <Card>
      <Card.Header>
        <Card.Title>关于应用</Card.Title>
        <Card.Description>ProxyPanel 客户端信息</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <p className="text-sm leading-relaxed text-zinc-500 dark:text-zinc-400">
          基于 sing-box 内核的 Android 代理客户端。
        </p>

        <AboutInfo />

        <div className="flex items-center justify-between gap-4">
          <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">核心引擎</span>
          <span className="text-right font-mono text-sm text-muted">
            {coreVersion ? `sing-box ${coreVersion}` : versionLoading ? "读取中…" : "sing-box（版本未知）"}
          </span>
        </div>

        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">编译 tags（构建配置）</span>
          <p className="break-all font-mono text-xs leading-relaxed text-zinc-500 dark:text-zinc-400">
            {SING_BOX_BUILD_TAGS.join(", ")}
          </p>
        </div>
      </Card.Content>
    </Card>
  );
}
