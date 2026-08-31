import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Checkbox, Input, Label, ListBox, Modal, Select } from "@heroui/react";
import type { LocalRuleInput, LocalRuleView } from "../../api";
import { RULE_ACTIONS } from "./types";

export interface RuleEditModalProps {
  isOpen: boolean;
  onClose: () => void;
  initial?: LocalRuleView | null;
  isAndroid: boolean;
  onSave: (rule: LocalRuleInput) => void;
}

export function RuleEditModal({ isOpen, onClose, initial, isAndroid, onSave }: RuleEditModalProps) {
  const [matchType, setMatchType] = useState("domain");
  const [target, setTarget] = useState("");
  const [action, setAction] = useState("proxy");
  const [name, setName] = useState("");
  const [note, setNote] = useState("");
  const [noResolve, setNoResolve] = useState(false);
  const [invert, setInvert] = useState(false);

  useEffect(() => {
    if (isOpen && initial) {
      setMatchType(initial.match_type);
      setTarget(initial.target);
      setAction(initial.action);
      setName(initial.name);
      setNote(initial.note);
      setNoResolve(initial.no_resolve);
      setInvert(initial.invert);
    } else if (isOpen) {
      setMatchType("domain");
      setTarget("");
      setAction("proxy");
      setName("");
      setNote("");
      setNoResolve(false);
      setInvert(false);
    }
  }, [isOpen, initial]);

  const matchTypeOptions = useMemo(() => {
    const base = [
      { id: "domain", label: "域名 (domain)" },
      { id: "domain_suffix", label: "域名后缀 (domain_suffix)" },
      { id: "domain_keyword", label: "域名关键词 (domain_keyword)" },
      { id: "ip_cidr", label: "IP 段 (ip_cidr)" },
      { id: "source_ip_cidr", label: "源 IP 段 (source_ip_cidr)" },
      { id: "rule_set", label: "规则集 (rule_set)" },
      { id: "port", label: "端口 (port)" },
      { id: "final", label: "最终规则 (final)" },
    ];
    if (isAndroid) {
      return [...base, { id: "app_package", label: "应用包名 (app_package)" }];
    }
    return [...base, { id: "process_name", label: "进程名 (process_name)" }];
  }, [isAndroid]);

  const handleSave = useCallback(() => {
    const now = Math.floor(Date.now() / 1000);
    onSave({
      id: initial?.id ?? crypto.randomUUID(),
      name: name.trim(),
      enabled: initial?.enabled ?? true,
      match_type: matchType,
      target: matchType === "final" ? "" : target.trim(),
      action,
      no_resolve: noResolve,
      invert,
      note: note.trim(),
      created_at: initial?.created_at ?? now,
      sort_order: initial?.sort_order ?? 0,
    });
    onClose();
  }, [initial, matchType, target, action, name, noResolve, invert, note, onSave, onClose]);

  const isFinal = matchType === "final";
  const canSave = isFinal ? true : target.trim().length > 0;

  return (
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[480px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{initial ? "编辑规则" : "新增规则"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex flex-col gap-4">
            <div className="flex flex-col gap-1">
              <Label>匹配类型</Label>
              <Select
                aria-label="匹配类型"
                value={matchType}
                onChange={(value) => setMatchType(String(value ?? "domain"))}
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {matchTypeOptions.map((opt) => (
                      <ListBox.Item key={opt.id} id={opt.id} textValue={opt.label}>
                        {opt.label}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>

            {!isFinal && (
              <div className="flex flex-col gap-1">
                <Label htmlFor="rule-target">匹配目标</Label>
                <Input
                  id="rule-target"
                  aria-label="匹配目标"
                  value={target}
                  onChange={(e) => setTarget(e.target.value)}
                  placeholder={matchType === "rule_set" ? "规则集 community_id" : "例如：googleapis.com"}
                  fullWidth
                />
              </div>
            )}

            <div className="flex flex-col gap-1">
              <Label>路由动作</Label>
              <Select
                aria-label="路由动作"
                value={action}
                onChange={(value) => setAction(String(value ?? "proxy"))}
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {RULE_ACTIONS.map((opt) => (
                      <ListBox.Item key={opt.id} id={opt.id} textValue={opt.label}>
                        {opt.label}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>

            <div className="flex flex-col gap-1">
              <Label htmlFor="rule-name">规则名称（可选）</Label>
              <Input
                id="rule-name"
                aria-label="规则名称"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="留空则自动生成摘要"
                fullWidth
              />
            </div>

            <div className="rounded-lg border border-border/40 p-3">
              <span className="text-xs font-medium text-muted">高级选项</span>
              <div className="mt-2 flex flex-col gap-2">
                <Checkbox isSelected={noResolve} onChange={(next) => setNoResolve(next)}>
                  <Checkbox.Content>
                    <Checkbox.Control>
                      <Checkbox.Indicator />
                    </Checkbox.Control>
                    跳过 DNS 解析 (no-resolve)
                  </Checkbox.Content>
                </Checkbox>
                <Checkbox isSelected={invert} onChange={(next) => setInvert(next)}>
                  <Checkbox.Content>
                    <Checkbox.Control>
                      <Checkbox.Indicator />
                    </Checkbox.Control>
                    反选 (invert)
                  </Checkbox.Content>
                </Checkbox>
                <div className="flex flex-col gap-1">
                  <Label htmlFor="rule-note">备注</Label>
                  <Input
                    id="rule-note"
                    aria-label="备注"
                    value={note}
                    onChange={(e) => setNote(e.target.value)}
                    placeholder="可选备注"
                    fullWidth
                  />
                </div>
              </div>
            </div>
          </Modal.Body>
          <Modal.Footer>
            <Button slot="close" variant="tertiary" onPress={onClose}>
              取消
            </Button>
            <Button variant="primary" isDisabled={!canSave} onPress={handleSave}>
              保存
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
