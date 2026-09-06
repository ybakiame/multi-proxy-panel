import { useState } from "react";
import { ChevronDownIcon, TrashIcon } from "@heroicons/react/24/outline";
import { Button, ListBox, Modal, Select, Switch } from "@heroui/react";
import type { LocalRuleInput, LocalRuleView } from "@pp/client-core";
import { RULE_ACTIONS } from "@pp/client-core";

/** 移动可编辑匹配类型：与 desktop RuleEditModal 对齐但按平台收敛——不含 `process_name`（仅 desktop），app_package 标注 Android 专属。 */
const MATCH_TYPE_OPTIONS = [
  { id: "domain", label: "域名" },
  { id: "domain_suffix", label: "域名后缀" },
  { id: "domain_keyword", label: "域名关键词" },
  { id: "ip_cidr", label: "IP 段" },
  { id: "source_ip_cidr", label: "源 IP 段" },
  { id: "rule_set", label: "规则集" },
  { id: "app_package", label: "应用包名" },
  { id: "port", label: "端口" },
  { id: "final", label: "最终规则" },
];

/** 目标输入占位（final 隐藏输入；未命中 key 的类型给通用占位）。 */
const TARGET_PLACEHOLDER: Record<string, string> = {
  domain: "例如：example.com",
  domain_suffix: "例如：google.com",
  domain_keyword: "例如：google",
  ip_cidr: "例如：1.2.3.0/24",
  source_ip_cidr: "例如：10.0.0.0/8",
  rule_set: "规则集 community_id",
  app_package: "例如：com.android.chrome",
  port: "例如：443",
};

/** 目标输入下方辅助说明（仅选中类型有提示时展示）。 */
const TARGET_HINT: Record<string, string> = {
  rule_set: "填入规则集列表内可用订阅的 community_id",
  app_package: "按 Android 应用包名匹配（仅 Android 生效）",
  port: "匹配目标端口；也支持端口段如 1000:2000",
};

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

interface RuleEditSheetProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该规则（表单预填）。 */
  editing: LocalRuleView | null;
  onClose: () => void;
  /** 保存（新建/编辑共用）；返回是否成功——成功才收起 Sheet，失败保留现场供重试。 */
  onSave: (rule: LocalRuleInput) => Promise<boolean>;
  /** 编辑模式点「删除规则」：父层收起 Sheet 并弹 AlertDialog 确认。 */
  onDeleteRequest: (rule: LocalRuleView) => void;
}

/** 底部 Sheet 内的高级选项开关行。 */
function SheetSwitchRow({
  label,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <div className="flex min-h-11 items-center justify-between gap-3">
      <span className="min-w-0 flex-1 text-sm text-foreground">{label}</span>
      <Switch aria-label={label} isSelected={checked} isDisabled={disabled} onChange={(next) => onChange(next)}>
        <Switch.Content>
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
        </Switch.Content>
      </Switch>
    </div>
  );
}

/**
 * 规则编辑底部 Sheet（ADR-0003 M5.4）。
 *
 * 校验与桌面 RuleEditModal 对齐：非 final 必须填目标（trim 非空）才可保存；
 * final 隐藏目标输入且保存时目标写空串；名称/备注 trim。保存由父层统一
 * persist 落盘（已存在替换 / 新建追加 sort_order），成功后 toast 并收起。
 */
export function RuleEditSheet({ isOpen, editing, onClose, onSave, onDeleteRequest }: RuleEditSheetProps) {
  const [matchType, setMatchType] = useState("domain");
  const [target, setTarget] = useState("");
  const [action, setAction] = useState("proxy");
  const [name, setName] = useState("");
  const [note, setNote] = useState("");
  const [noResolve, setNoResolve] = useState(false);
  const [invert, setInvert] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [prevKey, setPrevKey] = useState<string | null>(null);

  // open 切换（新建空表单 / 编辑预填）时同步表单初始值（adjust-state-during-render，
  // 替代 effect 同步 setState，同 SubscriptionFormSheet）。
  const key = isOpen ? (editing?.id ?? "__new__") : null;
  if (key !== null && key !== prevKey) {
    setPrevKey(key);
    setMatchType(editing?.match_type ?? "domain");
    setTarget(editing?.target ?? "");
    setAction(editing?.action ?? "proxy");
    setName(editing?.name ?? "");
    setNote(editing?.note ?? "");
    setNoResolve(editing?.no_resolve ?? false);
    setInvert(editing?.invert ?? false);
    setAdvancedOpen(false);
    setSaving(false);
  }

  const isFinal = matchType === "final";
  const canSave = isFinal ? true : target.trim().length > 0;

  const handleSave = async () => {
    if (!canSave || saving) return;
    setSaving(true);
    const now = Math.floor(Date.now() / 1000);
    const rule: LocalRuleInput = {
      id: editing?.id ?? crypto.randomUUID(),
      name: name.trim(),
      enabled: editing?.enabled ?? true,
      match_type: matchType,
      target: isFinal ? "" : target.trim(),
      action,
      no_resolve: noResolve,
      invert,
      note: note.trim(),
      created_at: editing?.created_at ?? now,
      sort_order: editing?.sort_order ?? 0,
    };
    const ok = await onSave(rule);
    setSaving(false);
    if (ok) {
      onClose();
    }
  };

  return (
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      isDismissable
    >
      <Modal.Container placement="bottom">
        <Modal.Dialog>
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{editing ? "编辑规则" : "添加规则"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            {/* 匹配类型 */}
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-medium text-foreground">匹配类型</span>
              <Select
                aria-label="匹配类型"
                value={matchType}
                onChange={(value) => setMatchType(String(value ?? "domain"))}
                isDisabled={saving}
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {MATCH_TYPE_OPTIONS.map((opt) => (
                      <ListBox.Item key={opt.id} id={opt.id} textValue={opt.label}>
                        {opt.label}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
              {matchType === "app_package" && (
                <span className="text-xs text-muted">应用包名匹配为 Android 专属能力</span>
              )}
            </div>

            {/* 匹配目标（final 隐藏） */}
            {!isFinal && (
              <div className="flex flex-col gap-1.5">
                <label htmlFor="rule-target" className="text-sm font-medium text-foreground">
                  匹配目标
                </label>
                <input
                  id="rule-target"
                  aria-required="true"
                  aria-label="匹配目标"
                  value={target}
                  onChange={(event) => setTarget(event.target.value)}
                  placeholder={TARGET_PLACEHOLDER[matchType] ?? "请输入目标值"}
                  autoCapitalize="none"
                  autoCorrect="off"
                  spellCheck={false}
                  disabled={saving}
                  className={inputClass}
                />
                {TARGET_HINT[matchType] && <span className="text-xs text-muted">{TARGET_HINT[matchType]}</span>}
              </div>
            )}

            {/* 路由动作分段控件 */}
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-medium text-foreground">路由动作</span>
              <fieldset className="flex min-w-0 gap-1 rounded-xl border border-border/60 bg-surface-secondary/40 p-1">
                <legend className="sr-only">路由动作</legend>
                {RULE_ACTIONS.map((opt) => (
                  <Button
                    key={opt.id}
                    variant={action === opt.id ? "primary" : "secondary"}
                    size="sm"
                    className="min-h-10 flex-1"
                    isDisabled={saving}
                    onPress={() => setAction(opt.id)}
                  >
                    {opt.label}
                  </Button>
                ))}
              </fieldset>
            </div>

            {/* 高级折叠段（默认收起） */}
            <div className="flex flex-col gap-3 rounded-xl border border-border/40 p-3">
              <button
                type="button"
                aria-expanded={advancedOpen}
                disabled={saving}
                onClick={() => setAdvancedOpen((open) => !open)}
                className="flex min-h-10 w-full items-center justify-between gap-2 text-left"
              >
                <span className="text-sm font-medium text-foreground">高级选项</span>
                <ChevronDownIcon
                  aria-hidden="true"
                  className={`size-5 text-muted transition-transform ${advancedOpen ? "rotate-180" : ""}`}
                />
              </button>
              {advancedOpen && (
                <div className="flex flex-col gap-3">
                  <label htmlFor="rule-name" className="flex flex-col gap-1.5">
                    <span className="text-sm font-medium text-foreground">规则名称（可选）</span>
                    <input
                      id="rule-name"
                      aria-label="规则名称"
                      value={name}
                      onChange={(event) => setName(event.target.value)}
                      placeholder="留空则自动生成摘要"
                      disabled={saving}
                      className={inputClass}
                    />
                  </label>
                  <SheetSwitchRow
                    label="跳过 DNS 解析 (no-resolve)"
                    checked={noResolve}
                    disabled={saving}
                    onChange={(next) => setNoResolve(next)}
                  />
                  <SheetSwitchRow
                    label="反选 (invert)"
                    checked={invert}
                    disabled={saving}
                    onChange={(next) => setInvert(next)}
                  />
                  <label htmlFor="rule-note" className="flex flex-col gap-1.5">
                    <span className="text-sm font-medium text-foreground">备注</span>
                    <input
                      id="rule-note"
                      aria-label="备注"
                      value={note}
                      onChange={(event) => setNote(event.target.value)}
                      placeholder="可选备注"
                      disabled={saving}
                      className={inputClass}
                    />
                  </label>
                </div>
              )}
            </div>

            {/* 编辑模式删除入口 */}
            {editing && (
              <Button
                variant="danger"
                className="min-h-12 w-full"
                isDisabled={saving}
                onPress={() => onDeleteRequest(editing)}
              >
                <TrashIcon className="size-4" aria-hidden="true" />
                删除规则
              </Button>
            )}
          </Modal.Body>
          <Modal.Footer>
            <Button variant="tertiary" className="min-h-12 flex-1" isDisabled={saving} onPress={onClose}>
              取消
            </Button>
            <Button
              variant="primary"
              className="min-h-12 flex-1"
              isDisabled={!canSave || saving}
              isPending={saving}
              onPress={() => void handleSave()}
            >
              保存
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
