import { BackHeader } from "../../components/BackHeader";
import { AboutCard } from "./AboutCard";

/**
 * 关于子页（路由 `/settings/about`）。
 *
 * 迁移原设置主页关于分组：应用名 + 说明 + 核心引擎（sing-box 版本 / 编译 tags）。
 */
export default function AboutPage() {
  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="关于" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        <AboutCard />
      </div>
    </div>
  );
}
