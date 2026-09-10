import { Card } from "@heroui/react";
import { useCoreVersion } from "@pp/client-core";
import { SING_BOX_BUILD_TAGS } from "../../coreBuildInfo";

/**
 * 关于（ADR-0003 M5.3）：应用名 + 说明 + 核心引擎信息。
 *
 * sing-box 版本经 `useCoreVersion`（libbox.Version() 运行时 API）动态读取，
 * 编译 tags 无运行时导出，取本地常量 `SING_BOX_BUILD_TAGS`（升级 sing-box 后
 * 需同步 build-panel-core.sh，见常量文件注释）。
 */
export function AboutCard() {
  const { data: coreVersion, isLoading: versionLoading } = useCoreVersion();

  return (
    <Card>
      <Card.Header>
        <Card.Title>ProxyPanel Mobile</Card.Title>
        <Card.Description>关于应用</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <p className="text-sm leading-relaxed text-muted">
          基于 sing-box 内核的 Android 代理客户端：同步 ProxyPanel Hub 订阅并本地合成配置， 经系统 VPN 转发流量。
        </p>

        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-foreground">核心引擎</span>
          <span className="font-mono text-sm text-muted">
            {coreVersion ? `sing-box ${coreVersion}` : versionLoading ? "读取中…" : "sing-box（版本未知）"}
          </span>
        </div>

        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-foreground">编译 tags</span>
          <p className="font-mono text-xs leading-relaxed break-all text-muted">{SING_BOX_BUILD_TAGS.join(", ")}</p>
        </div>
      </Card.Content>
    </Card>
  );
}
