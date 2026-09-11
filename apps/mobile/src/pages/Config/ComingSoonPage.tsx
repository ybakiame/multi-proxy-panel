import { BackHeader } from "../../components/BackHeader";

/**
 * 配置切片占位子页（ADR-0005 §3.3 P0-4a）。
 *
 * 本阶段只搭路由 / 导航骨架，DNS 与自定义出站的表单由后续阶段实现；
 * 页面仅渲染返回头 + 说明文案，避免过度设计。
 */
export default function ComingSoonPage({ title }: { title: string }) {
  return (
    <div className="flex min-h-full flex-col">
      <BackHeader title={title} />
      <div className="flex flex-1 flex-col items-center justify-center gap-2 px-6 py-16 text-center">
        <span className="text-sm font-medium text-foreground">{title}设置建设中</span>
        <span className="text-xs text-muted">该配置切片将在后续版本开放，敬请期待。</span>
      </div>
    </div>
  );
}
