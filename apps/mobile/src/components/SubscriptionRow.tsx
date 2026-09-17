import { ArrowPathIcon, PencilSquareIcon, TrashIcon } from "@heroicons/react/24/outline";
import { Chip, Spinner, Switch } from "@heroui/react";
import type { ReactNode } from "react";
import type { SubscriptionView } from "@pp/client-core";

interface SubscriptionRowProps {
  sub: SubscriptionView;
  /** 是否为当前生效订阅（`config.active_subscription_id` 匹配）。 */
  isActive: boolean;
  /** 本卡任意写/刷新操作进行中或全量刷新中：禁用整卡交互。 */
  busy: boolean;
  /** 本卡单条刷新进行中（刷新按钮显示 loading）。 */
  refreshing: boolean;
  onActivate: () => void;
  onToggle: () => void;
  onRefresh: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

/** 操作图标按钮（触达区 44×44）。 */
function IconButton({
  label,
  danger,
  onPress,
  disabled,
  children,
}: {
  label: string;
  danger?: boolean;
  onPress: () => void;
  disabled: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={disabled ? undefined : onPress}
      className={`flex size-11 shrink-0 items-center justify-center rounded-lg transition-opacity active:opacity-70 disabled:cursor-default disabled:opacity-40 ${
        danger ? "text-danger" : "text-muted"
      }`}
    >
      {children}
    </button>
  );
}

/**
 * 订阅卡片（ADR-0003 M5.6，移动单列布局）。
 *
 * - 卡片主体点击 → 设为生效（停用订阅由页面层 toast 引导先启用）；
 * - 展示：名称（截断）+「生效中」chip、URL（截断）、节点数、最近拉取失败提示（若有）；
 * - 右上启用 Switch；底部操作：刷新（busy 转圈）/ 编辑 / 删除（危险色）；
 * - 生效中高亮边框/背景（对齐 ProxyNodeItem 选中态）。
 */
export function SubscriptionRow({
  sub,
  isActive,
  busy,
  refreshing,
  onActivate,
  onToggle,
  onRefresh,
  onEdit,
  onDelete,
}: SubscriptionRowProps) {
  const controlsDisabled = busy || refreshing;
  return (
    <div
      className={`flex flex-col overflow-hidden rounded-xl border transition-colors ${
        isActive ? "border-accent/60 bg-accent/5" : "border-border/60 bg-surface"
      }`}
    >
      <div className="flex items-center gap-1 px-2">
        <button
          type="button"
          onClick={controlsDisabled ? undefined : onActivate}
          disabled={controlsDisabled}
          className="flex min-w-0 flex-1 flex-col gap-0.5 py-2.5 pl-1.5 pr-1 text-left disabled:cursor-default"
        >
          <span className="flex min-w-0 items-center gap-1.5">
            <span className="min-w-0 truncate text-sm font-medium text-foreground">{sub.name}</span>
            {isActive && (
              <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                生效中
              </Chip>
            )}
          </span>
          <span className="truncate font-mono text-xs text-muted" title={sub.url}>
            {sub.url}
          </span>
          {sub.error && <span className="line-clamp-2 text-xs text-warning">{sub.error}</span>}
        </button>
        <Switch
          aria-label={`启用 ${sub.name}`}
          isSelected={sub.enabled}
          isDisabled={controlsDisabled}
          onChange={() => onToggle()}
          className="shrink-0 px-1.5"
        >
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
          </Switch.Content>
        </Switch>
      </div>
      <div className="flex items-center gap-1 border-t border-border/40 pl-3">
        <span className="mr-auto py-1 text-xs text-muted">{sub.node_count} 个节点</span>
        <IconButton label={`刷新 ${sub.name}`} disabled={controlsDisabled} onPress={onRefresh}>
          {refreshing ? (
            <Spinner size="sm" color="accent" aria-hidden="true" />
          ) : (
            <ArrowPathIcon className="size-5" aria-hidden="true" />
          )}
        </IconButton>
        <IconButton label={`编辑 ${sub.name}`} disabled={controlsDisabled} onPress={onEdit}>
          <PencilSquareIcon className="size-5" aria-hidden="true" />
        </IconButton>
        <IconButton label={`删除 ${sub.name}`} danger disabled={controlsDisabled} onPress={onDelete}>
          <TrashIcon className="size-5" aria-hidden="true" />
        </IconButton>
      </div>
    </div>
  );
}
