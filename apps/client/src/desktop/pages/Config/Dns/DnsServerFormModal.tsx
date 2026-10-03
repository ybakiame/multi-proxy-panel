import { useState } from "react";
import { Button, Input, Label, ListBox, Modal, Select } from "@heroui/react";
import { TrashIcon } from "@heroicons/react/24/outline";
import type { DnsServer, DnsServerType } from "@pp/client-core";
import {
  DEFAULT_FAKEIP_INET4_RANGE,
  DNS_SERVER_TYPE_OPTIONS,
  parsePortDraft,
  validateDnsServerForm,
} from "@pp/client-core";

interface DnsServerFormModalProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该服务器（表单预填）。 */
  editing: DnsServer | null;
  /** 除自身外的已有 tag（编辑时排除自身），用于唯一性校验。 */
  otherTags: readonly string[];
  onClose: () => void;
  /** 保存（仅更新内存草稿，落盘由页面统一执行）。 */
  onSave: (server: DnsServer) => void;
  /** 编辑模式点「删除服务器」：父层收起 Modal 并弹 AlertDialog 确认。 */
  onDeleteRequest: (server: DnsServer) => void;
}

/**
 * DNS 服务器编辑弹窗（桌面端；语义对齐移动端 `DnsServerFormSheet`）。
 *
 * 字段：tag / name（可选展示名）/ type / server / server_port / detour /
 * domain_resolver；`local` 类型隐藏 server 与 port，`fakeip` 类型改为网段字段。
 * 校验（tag 唯一无空白、非 local server 必填、端口 1-65535、CIDR 网段）即时进行，
 * 非法时禁用保存并给出行内错误。
 */
export function DnsServerFormModal({
  isOpen,
  editing,
  otherTags,
  onClose,
  onSave,
  onDeleteRequest,
}: DnsServerFormModalProps) {
  return (
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[520px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{editing ? "编辑 DNS 服务器" : "添加 DNS 服务器"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            {/* key 重挂载保证 editing 切换时表单状态重置（同 RuleEditModal 模式） */}
            <DnsServerForm
              key={editing?.tag ?? "__new__"}
              editing={editing}
              otherTags={otherTags}
              onSave={onSave}
              onClose={onClose}
              onDeleteRequest={onDeleteRequest}
            />
          </Modal.Body>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}

function DnsServerForm({
  editing,
  otherTags,
  onSave,
  onClose,
  onDeleteRequest,
}: {
  editing: DnsServer | null;
  otherTags: readonly string[];
  onSave: (server: DnsServer) => void;
  onClose: () => void;
  onDeleteRequest: (server: DnsServer) => void;
}) {
  const [tag, setTag] = useState(editing?.tag ?? "");
  const [name, setName] = useState(editing?.name ?? "");
  const [serverType, setServerType] = useState<DnsServerType>(editing?.server_type ?? "udp");
  const [server, setServer] = useState(editing?.server ?? "");
  const [port, setPort] = useState(editing?.server_port != null ? String(editing.server_port) : "");
  const [inet4Range, setInet4Range] = useState(editing?.inet4_range ?? "");
  const [inet6Range, setInet6Range] = useState(editing?.inet6_range ?? "");
  const [detour, setDetour] = useState(editing?.detour ?? "");
  const [domainResolver, setDomainResolver] = useState(editing?.domain_resolver ?? "");

  const isLocal = serverType === "local";
  const isFakeip = serverType === "fakeip";
  // FakeIP 无拨号字段（server / port / detour / domain_resolver 均由 Rust 渲染时忽略）。
  const showDialFields = !isLocal && !isFakeip;
  const errors = validateDnsServerForm({ tag, serverType, server, port, inet4Range, inet6Range }, otherTags);
  const canSave =
    errors.tag === null &&
    errors.server === null &&
    errors.port === null &&
    errors.inet4 === null &&
    errors.inet6 === null;

  const handleSave = () => {
    if (!canSave) return;
    onSave({
      tag: tag.trim(),
      name: name.trim(),
      enabled: editing?.enabled ?? true,
      server: showDialFields ? server.trim() : "",
      server_type: serverType,
      server_port: showDialFields ? parsePortDraft(port).value : null,
      inet4_range: isFakeip ? inet4Range.trim() : "",
      inet6_range: isFakeip ? inet6Range.trim() : "",
      detour: showDialFields ? detour.trim() : "",
      strategy: null,
      domain_resolver: showDialFields ? domainResolver.trim() : "",
    });
    onClose();
  };

  return (
    <>
      {/* tag */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="dns-server-tag">tag</Label>
        <Input
          id="dns-server-tag"
          aria-label="服务器 tag"
          aria-required="true"
          value={tag}
          onChange={(event) => setTag(event.target.value)}
          placeholder="例如：local-dns"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          className="font-mono"
        />
        {errors.tag ? (
          <span className="text-xs text-amber-500">{errors.tag}</span>
        ) : (
          <span className="text-xs text-muted">切片内唯一，供规则与 final 引用</span>
        )}
      </div>

      {/* name（可选展示名，仅 UI 展示，不写入核心配置） */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="dns-server-name">名称（可选）</Label>
        <Input
          id="dns-server-name"
          aria-label="服务器名称"
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder="例如：AliDNS"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
        />
      </div>

      {/* type */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="dns-server-type">类型</Label>
        <Select
          id="dns-server-type"
          aria-label="服务器类型"
          value={serverType}
          onChange={(key) => setServerType(String(key) as DnsServerType)}
          fullWidth
        >
          <Select.Trigger>
            <Select.Value />
            <Select.Indicator />
          </Select.Trigger>
          <Select.Popover>
            <ListBox>
              {DNS_SERVER_TYPE_OPTIONS.map((option) => (
                <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                  {option.label}
                  <ListBox.ItemIndicator />
                </ListBox.Item>
              ))}
            </ListBox>
          </Select.Popover>
        </Select>
      </div>

      {/* server / port（local / fakeip 隐藏） */}
      {showDialFields && (
        <>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="dns-server-address">服务器地址</Label>
            <Input
              id="dns-server-address"
              aria-label="服务器地址"
              aria-required="true"
              value={server}
              onChange={(event) => setServer(event.target.value)}
              placeholder="例如：1.1.1.1 或 dns.example.com"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              className="font-mono"
            />
            {errors.server ? (
              <span className="text-xs text-amber-500">{errors.server}</span>
            ) : (
              <span className="text-xs text-muted">IP 或域名</span>
            )}
          </div>

          <div className="flex flex-col gap-1.5">
            <Label htmlFor="dns-server-port">端口</Label>
            <Input
              id="dns-server-port"
              aria-label="服务器端口"
              inputMode="numeric"
              value={port}
              onChange={(event) => setPort(event.target.value)}
              placeholder="留空使用类型默认端口"
              className="font-mono"
            />
            {errors.port && <span className="text-xs text-amber-500">{errors.port}</span>}
          </div>
        </>
      )}

      {/* fakeip 网段（仅 fakeip） */}
      {isFakeip && (
        <>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="dns-server-inet4">IPv4 网段</Label>
            <Input
              id="dns-server-inet4"
              aria-label="FakeIP IPv4 网段"
              value={inet4Range}
              onChange={(event) => setInet4Range(event.target.value)}
              placeholder={`留空使用默认 ${DEFAULT_FAKEIP_INET4_RANGE}`}
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              className="font-mono"
            />
            {errors.inet4 ? (
              <span className="text-xs text-amber-500">{errors.inet4}</span>
            ) : (
              <span className="text-xs text-muted">虚拟 IPv4 地址池，CIDR 格式</span>
            )}
          </div>

          <div className="flex flex-col gap-1.5">
            <Label htmlFor="dns-server-inet6">IPv6 网段</Label>
            <Input
              id="dns-server-inet6"
              aria-label="FakeIP IPv6 网段"
              value={inet6Range}
              onChange={(event) => setInet6Range(event.target.value)}
              placeholder="留空则不返回虚拟 IPv6 地址"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              className="font-mono"
            />
            {errors.inet6 && <span className="text-xs text-amber-500">{errors.inet6}</span>}
          </div>
        </>
      )}

      {/* detour（fakeip 隐藏） */}
      {!isFakeip && (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="dns-server-detour">出站 (detour)</Label>
          <Input
            id="dns-server-detour"
            aria-label="出站 detour"
            value={detour}
            onChange={(event) => setDetour(event.target.value)}
            placeholder="留空为默认直连，例如：proxy"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            className="font-mono"
          />
        </div>
      )}

      {/* domain_resolver（fakeip 隐藏） */}
      {!isFakeip && (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="dns-server-resolver">域名解析器</Label>
          <Input
            id="dns-server-resolver"
            aria-label="域名解析器"
            value={domainResolver}
            onChange={(event) => setDomainResolver(event.target.value)}
            placeholder="用于解析本服务器域名的 server tag"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            className="font-mono"
          />
        </div>
      )}

      <div className="flex gap-2">
        {editing && (
          <Button variant="danger" onPress={() => onDeleteRequest(editing)}>
            <TrashIcon className="size-4" aria-hidden="true" />
            删除服务器
          </Button>
        )}
        <div className="flex-1" />
        <Button variant="tertiary" onPress={onClose}>
          取消
        </Button>
        <Button variant="primary" isDisabled={!canSave} onPress={handleSave}>
          保存
        </Button>
      </div>
    </>
  );
}
