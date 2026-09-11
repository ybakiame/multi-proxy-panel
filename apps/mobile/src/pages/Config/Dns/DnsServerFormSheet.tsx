import { useState } from "react";
import { TrashIcon } from "@heroicons/react/24/outline";
import { Button, Modal } from "@heroui/react";
import type { DnsServer, DnsServerType } from "@pp/client-core";
import { MobileSelectSheet } from "../../../components/MobileSelectSheet";
import { DNS_SERVER_TYPE_OPTIONS, parsePortDraft, validateDnsServerForm } from "./dnsUtils";

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

interface DnsServerFormSheetProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该服务器（表单预填）。 */
  editing: DnsServer | null;
  /** 除自身外的已有 tag（编辑时排除自身），用于唯一性校验。 */
  otherTags: readonly string[];
  onClose: () => void;
  /** 保存（仅更新内存草稿，落盘由页面统一执行）。 */
  onSave: (server: DnsServer) => void;
  /** 编辑模式点「删除服务器」：父层收起 Sheet 并弹 AlertDialog 确认。 */
  onDeleteRequest: (server: DnsServer) => void;
}

/**
 * DNS 服务器编辑底部 Sheet（ADR-0005 P0-4b）。
 *
 * 字段：tag / type / server / server_port / detour / domain_resolver；
 * `local` 类型隐藏 server 与 port。校验（tag 唯一无空白、非 local server 必填、
 * 端口 1-65535）即时进行，非法时禁用保存并给出行内错误。
 *
 * 不提供 per-server 解析策略：sing-box 1.12+ 新 DNS 服务器格式已无该字段，
 * schema 中的 `strategy` 仅作前向兼容保留（保存恒写 `null`）。
 */
export function DnsServerFormSheet({
  isOpen,
  editing,
  otherTags,
  onClose,
  onSave,
  onDeleteRequest,
}: DnsServerFormSheetProps) {
  const [tag, setTag] = useState("");
  const [serverType, setServerType] = useState<DnsServerType>("udp");
  const [server, setServer] = useState("");
  const [port, setPort] = useState("");
  const [detour, setDetour] = useState("");
  const [domainResolver, setDomainResolver] = useState("");
  const [prevKey, setPrevKey] = useState<string | null>(null);

  // open 切换（新建空表单 / 编辑预填）时同步表单初始值（adjust-state-during-render）。
  const key = isOpen ? (editing?.tag ?? "__new__") : null;
  if (key !== null && key !== prevKey) {
    setPrevKey(key);
    setTag(editing?.tag ?? "");
    setServerType(editing?.server_type ?? "udp");
    setServer(editing?.server ?? "");
    setPort(editing?.server_port != null ? String(editing.server_port) : "");
    setDetour(editing?.detour ?? "");
    setDomainResolver(editing?.domain_resolver ?? "");
  }

  const isLocal = serverType === "local";
  const errors = validateDnsServerForm({ tag, serverType, server, port }, otherTags);
  const canSave = errors.tag === null && errors.server === null && errors.port === null;

  const handleSave = () => {
    if (!canSave) return;
    onSave({
      tag: tag.trim(),
      server: isLocal ? "" : server.trim(),
      server_type: serverType,
      server_port: parsePortDraft(port).value,
      detour: detour.trim(),
      strategy: null,
      domain_resolver: domainResolver.trim(),
    });
    onClose();
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
            <Modal.Heading>{editing ? "编辑 DNS 服务器" : "添加 DNS 服务器"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            {/* tag */}
            <div className="flex flex-col gap-1.5">
              <label htmlFor="dns-server-tag" className="text-sm font-medium text-foreground">
                tag
              </label>
              <input
                id="dns-server-tag"
                aria-label="服务器 tag"
                aria-required="true"
                value={tag}
                onChange={(event) => setTag(event.target.value)}
                placeholder="例如：local-dns"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                className={`${inputClass} font-mono`}
              />
              {errors.tag ? (
                <span className="text-xs text-warning">{errors.tag}</span>
              ) : (
                <span className="text-xs text-muted">切片内唯一，供规则与 final 引用</span>
              )}
            </div>

            {/* type */}
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-medium text-foreground">类型</span>
              <MobileSelectSheet
                label="服务器类型"
                value={serverType}
                onChange={(value) => setServerType(value as DnsServerType)}
                options={DNS_SERVER_TYPE_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
              />
            </div>

            {/* server / port（local 隐藏） */}
            {!isLocal && (
              <>
                <div className="flex flex-col gap-1.5">
                  <label htmlFor="dns-server-address" className="text-sm font-medium text-foreground">
                    服务器地址
                  </label>
                  <input
                    id="dns-server-address"
                    aria-label="服务器地址"
                    aria-required="true"
                    value={server}
                    onChange={(event) => setServer(event.target.value)}
                    placeholder="例如：1.1.1.1 或 dns.example.com"
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    className={`${inputClass} font-mono`}
                  />
                  {errors.server ? (
                    <span className="text-xs text-warning">{errors.server}</span>
                  ) : (
                    <span className="text-xs text-muted">IP 或域名</span>
                  )}
                </div>

                <div className="flex flex-col gap-1.5">
                  <label htmlFor="dns-server-port" className="text-sm font-medium text-foreground">
                    端口
                  </label>
                  <input
                    id="dns-server-port"
                    aria-label="服务器端口"
                    inputMode="numeric"
                    value={port}
                    onChange={(event) => setPort(event.target.value)}
                    placeholder="留空使用类型默认端口"
                    className={`${inputClass} font-mono`}
                  />
                  {errors.port && <span className="text-xs text-warning">{errors.port}</span>}
                </div>
              </>
            )}

            {/* detour */}
            <div className="flex flex-col gap-1.5">
              <label htmlFor="dns-server-detour" className="text-sm font-medium text-foreground">
                出站 (detour)
              </label>
              <input
                id="dns-server-detour"
                aria-label="出站 detour"
                value={detour}
                onChange={(event) => setDetour(event.target.value)}
                placeholder="留空为默认直连，例如：proxy"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                className={`${inputClass} font-mono`}
              />
            </div>

            {/* domain_resolver */}
            <div className="flex flex-col gap-1.5">
              <label htmlFor="dns-server-resolver" className="text-sm font-medium text-foreground">
                域名解析器
              </label>
              <input
                id="dns-server-resolver"
                aria-label="域名解析器"
                value={domainResolver}
                onChange={(event) => setDomainResolver(event.target.value)}
                placeholder="用于解析本服务器域名的 server tag"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                className={`${inputClass} font-mono`}
              />
            </div>

            {/* 编辑模式删除入口 */}
            {editing && (
              <Button variant="danger" className="min-h-12 w-full" onPress={() => onDeleteRequest(editing)}>
                <TrashIcon className="size-4" aria-hidden="true" />
                删除服务器
              </Button>
            )}
          </Modal.Body>
          <Modal.Footer>
            <Button variant="tertiary" className="min-h-12 flex-1" onPress={onClose}>
              取消
            </Button>
            <Button variant="primary" className="min-h-12 flex-1" isDisabled={!canSave} onPress={handleSave}>
              保存
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
