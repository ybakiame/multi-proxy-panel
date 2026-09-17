import { CheckIcon } from "@heroicons/react/24/outline";
import type { OutboundFormFields } from "./outboundForm";
import { SectionTitle, SelectField, SwitchRow, TextField } from "./OutboundField";
import { DEFAULT_URLTEST_URL, type GroupFormErrors, type GroupMemberCandidate } from "./groupForm";

interface GroupFieldsProps {
  fields: OutboundFormFields;
  errors: GroupFormErrors;
  /** 成员候选（静态订阅节点 + 切片节点 + direct，不含其它分组；内置动态分组的默认成员
   *  用全候选；内置静态分组成员可编辑，候选由页面收窄防循环）。 */
  candidates: GroupMemberCandidate[];
  /** 内置静态成员分组（global/final）：成员可编辑；内置动态分组（proxy/auto）隐藏成员。 */
  builtinMembersEditable?: boolean;
  /** 生效订阅是否存在节点缓存：false 时顶部引导先同步订阅。 */
  subscriptionCacheAvailable: boolean;
  /** 局部字段补丁更新（父层持有完整草稿）。 */
  onChange: (patch: Partial<OutboundFormFields>) => void;
}

/**
 * 分组出站（selector / urltest）字段区（ADR-0005 P0-4c）。
 *
 * 成员为触屏多选行（`min-h-12`，显示友好名 + tag）；候选不含其它分组（禁嵌套）。
 * selector 额外提供默认成员选择器，urltest 额外提供 url / interval / tolerance；
 * 两者共用 interrupt_exist_connections 开关。
 */
export function GroupFields({
  fields,
  errors,
  candidates,
  subscriptionCacheAvailable,
  builtinMembersEditable = false,
  onChange,
}: GroupFieldsProps) {
  const builtin = fields.builtin;
  /** 切换成员选中态；取消选中默认成员时同步清空 default（避免悬空）。 */
  const toggleMember = (tag: string) => {
    const selected = fields.members.includes(tag);
    const members = selected ? fields.members.filter((member) => member !== tag) : [...fields.members, tag];
    const patch: Partial<OutboundFormFields> = { members };
    if (selected && fields.groupDefault === tag) {
      patch.groupDefault = "";
    }
    onChange(patch);
  };

  // 内置动态分组（proxy/auto）：成员不可编辑，默认成员候选直接用全候选（成员由模板
  // 动态计算）；其余（自定义 / 内置静态）默认成员候选 = 已选成员。
  const memberOptions =
    builtin && !builtinMembersEditable
      ? candidates.map((candidate) => ({ value: candidate.value, label: candidate.label }))
      : fields.members.map((tag) => {
          const found = candidates.find((candidate) => candidate.value === tag);
          return { value: tag, label: found?.label ?? tag };
        });

  return (
    <>
      {!subscriptionCacheAvailable && !builtin && (
        <span className="text-xs text-muted">未找到订阅节点缓存，请先同步订阅</span>
      )}
      {builtin && <span className="text-xs text-muted">内置分组的成员列表由模板按订阅动态计算，不可编辑</span>}

      {/* 成员多选（内置分组隐藏） */}
      {!builtin && (
        <div className="flex flex-col gap-1.5">
          <SectionTitle>成员</SectionTitle>
          {candidates.length === 0 ? (
            <span className="text-xs text-muted">暂无可选成员</span>
          ) : (
            <div className="flex flex-col gap-1.5">
              {candidates.map((candidate) => {
                const selected = fields.members.includes(candidate.value);
                return (
                  <button
                    key={candidate.value}
                    type="button"
                    aria-pressed={selected}
                    onClick={() => toggleMember(candidate.value)}
                    className={`flex min-h-12 w-full items-center justify-between gap-3 rounded-xl border px-4 py-2 text-left transition-colors ${
                      selected ? "border-accent/50 bg-accent/5" : "border-border/70 active:bg-surface-secondary/60"
                    }`}
                  >
                    <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                      <span className="truncate text-sm font-medium text-foreground">{candidate.label}</span>
                      <span className="truncate font-mono text-xs text-muted">{candidate.value}</span>
                    </span>
                    {selected && <CheckIcon className="size-5 shrink-0 text-accent" aria-hidden="true" />}
                  </button>
                );
              })}
            </div>
          )}
          {errors.members && <span className="text-xs text-warning">{errors.members}</span>}
        </div>
      )}

      {/* selector 专属：默认成员 */}
      {fields.protocol === "selector" && (
        <SelectField
          label="默认成员（可选）"
          value={fields.groupDefault}
          onChange={(groupDefault) => onChange({ groupDefault })}
          options={memberOptions}
          placeholder="默认使用第一个成员"
          error={errors.defaultMember}
          hint="留空时使用成员列表的第一个"
        />
      )}

      {/* urltest 专属：url / interval / tolerance */}
      {fields.protocol === "urltest" && (
        <>
          <TextField
            id="outbound-group-url"
            label="测速 URL（可选）"
            value={fields.groupUrl}
            onChange={(groupUrl) => onChange({ groupUrl })}
            placeholder={DEFAULT_URLTEST_URL}
            error={errors.url}
            hint="留空使用核心默认 URL"
            mono
          />
          <TextField
            id="outbound-group-interval"
            label="测速间隔（可选）"
            value={fields.groupInterval}
            onChange={(groupInterval) => onChange({ groupInterval })}
            placeholder="如 3m"
            error={errors.interval}
            hint="格式如 3m / 30s / 1h，留空使用核心默认"
            mono
          />
          <TextField
            id="outbound-group-tolerance"
            label="容差 ms（可选）"
            inputMode="numeric"
            value={fields.groupTolerance}
            onChange={(groupTolerance) => onChange({ groupTolerance })}
            placeholder="0"
            error={errors.tolerance}
            hint="0-65535；0 使用核心默认（50ms）"
            mono
          />
        </>
      )}

      <SwitchRow
        label="切换成员时中断现有连接"
        ariaLabel="切换成员时中断现有连接"
        isSelected={fields.interruptExistConnections}
        onChange={(interruptExistConnections) => onChange({ interruptExistConnections })}
      />
    </>
  );
}
