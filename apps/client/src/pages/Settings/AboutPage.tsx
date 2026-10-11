import { SubPageShell } from "../../components/PageShell";
import { AboutCard } from "./AboutCard";
import ProductCard from "./ProductCard";
import { IS_MOBILE } from "@pp/ui";

/**
 * 关于子页（路由 `/settings/about`）。
 *
 * 迁移原设置主页关于分组：应用名 + 说明 + 核心引擎（sing-box 版本 / 编译 tags）。
 */
export default function AboutPage() {
  return (
    <SubPageShell title="关于">
      <ProductCard />
      {IS_MOBILE && <AboutCard />}
    </SubPageShell>
  );
}
