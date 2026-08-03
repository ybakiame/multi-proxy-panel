import { useCallback, useEffect, useState } from "react";
import { Alert, Button, Card, Chip, Input, Label, ListBox, Meter, Modal, Select, Switch, Table } from "@heroui/react";
import {
  addSubscription,
  listProfiles,
  listSubscriptions,
  refreshSubscription,
  removeSubscription,
  setSubscriptionEnabled,
  toErrorMessage,
  updateSubscription,
} from "../api";
import type { ProfileView, SubscriptionFormat, SubscriptionUserInfo, SubscriptionView } from "../api";
import ConfigPreviewModal from "../components/ConfigPreviewModal";
import { useAppStore } from "../store";

/** 字节数格式化为 GB（保留两位小数）。 */
function formatGb(bytes: number | null | undefined): string {
  if (bytes == null) {
    return "-";
  }
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

/** 已用流量 = download + upload；total 缺失时仅显示已用。 */
function usageText(info: SubscriptionUserInfo | null): string {
  if (!info) {
    return "-";
  }
  const used = (info.download ?? 0) + (info.upload ?? 0);
  if (info.total == null) {
    return formatGb(used);
  }
  return `${formatGb(used)} / ${formatGb(info.total)}`;
}

/** 用量百分比（0-100，total 缺失时按 0 处理）。 */
function usagePercent(info: SubscriptionUserInfo | null): number {
  if (!info || info.total == null || info.total <= 0) {
    return 0;
  }
  const used = (info.download ?? 0) + (info.upload ?? 0);
  return Math.min(100, Math.max(0, (used / info.total) * 100));
}

/** 到期时间戳（秒）转为本地日期；缺失返回占位符。 */
function formatExpire(expire: number | null | undefined): string {
  if (expire == null) {
    return "-";
  }
  const date = new Date(expire * 1000);
  return Number.isNaN(date.getTime()) ? "-" : date.toLocaleDateString();
}

/** 用量进度条颜色：超量红、>80% 黄、其余强调色。 */
function usageColor(percent: number): "accent" | "warning" | "danger" {
  if (percent >= 100) {
    return "danger";
  }
  if (percent >= 80) {
    return "warning";
  }
  return "accent";
}

/** 常用 UA 快捷选择（空串 = 默认 clash.meta）。 */
const UA_PRESETS = [
  { value: "", label: "默认" },
  { value: "clash.meta", label: "clash.meta" },
  { value: "clash-verge", label: "clash-verge" },
  { value: "sing-box", label: "sing-box" },
];

type OpResult = { sub: SubscriptionView; kind: "add" | "refresh" };

/** 订阅格式展示名：ClashYaml → Clash，SingBoxJson → sing-box。 */
function formatLabel(format: SubscriptionFormat): string {
  if (format === "ClashYaml") {
    return "Clash";
  }
  if (format === "SingBoxJson") {
    return "sing-box";
  }
  return "ShareLinks";
}

/** 订阅格式 Chip 配色：ShareLinks 强调色、ClashYaml 警告色、SingBoxJson 成功色。 */
function formatColor(format: SubscriptionFormat): "accent" | "warning" | "success" {
  if (format === "ClashYaml") {
    return "warning";
  }
  if (format === "SingBoxJson") {
    return "success";
  }
  return "accent";
}

/** 核心类型展示名：singbox → sing-box，mihomo → mihomo（用于关联覆写提示）。 */
function coreLabel(coreType: string | undefined): string {
  return coreType === "mihomo" ? "mihomo" : "sing-box";
}

/** 由订阅 format 推导适配核心；ShareLinks/空 返回 null（跟随全局核心）。 */
function subCoreType(format: SubscriptionFormat | null | undefined): "singbox" | "mihomo" | null {
  if (format === "ClashYaml") {
    return "mihomo";
  }
  if (format === "SingBoxJson") {
    return "singbox";
  }
  return null;
}

export default function Nodes() {
  const [subs, setSubs] = useState<SubscriptionView[]>([]);
  const [profiles, setProfiles] = useState<ProfileView[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<OpResult | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);

  // 添加订阅对话框
  const [addOpen, setAddOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newUrl, setNewUrl] = useState("");
  const [newUa, setNewUa] = useState("");
  // 关联覆写模板 id（"" = 不关联）
  const [newProfileId, setNewProfileId] = useState("");

  // 编辑订阅对话框（名称 / URL / UA / 关联覆写可改，UA 沿用添加快捷 chips）
  const [editSub, setEditSub] = useState<SubscriptionView | null>(null);
  const [editName, setEditName] = useState("");
  const [editUrl, setEditUrl] = useState("");
  const [editUa, setEditUa] = useState("");
  // 关联覆写模板 id（"" = 不关联）
  const [editProfileId, setEditProfileId] = useState("");

  // 配置预览弹窗：按指定订阅生成合成配置（忽略 enabled 状态）。
  const [previewSub, setPreviewSub] = useState<SubscriptionView | null>(null);

  // 当前客户端核心类型（来自全局 store 的设置页配置），用于格式不匹配提示与关联覆写过滤。
  const clientCoreType = useAppStore((state) => state.config?.core_type);
  const loadConfig = useAppStore((state) => state.loadConfig);

  const refreshSubs = useCallback(async () => {
    try {
      setSubs(await listSubscriptions());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  const loadProfiles = useCallback(async () => {
    try {
      setProfiles(await listProfiles());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refreshSubs();
    void loadProfiles();
  }, [refreshSubs, loadProfiles]);

  /** 当前客户端核心类型下可关联的覆写模板（带核心过滤；添加时订阅尚未拉取、无 format，跟随全局核心）。 */
  const coreProfiles = profiles.filter((profile) => profile.core_type === clientCoreType);

  /** 编辑中的订阅按自身 format 推导适配核心（未拉取/ShareLinks 时跟随全局核心）。 */
  const editCoreType = subCoreType(editSub?.format) ?? clientCoreType;
  /** 编辑弹窗按推导核心过滤覆写候选。 */
  const editCoreProfiles = profiles.filter((profile) => profile.core_type === editCoreType);

  const handleAdd = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const ua = newUa.trim();
      const sub = await addSubscription({
        name: newName.trim(),
        url: newUrl.trim(),
        user_agent: ua || undefined,
        // 「不关联」传 null（与后端 `None` 语义对齐）
        profile_id: newProfileId === "" ? null : newProfileId,
      });
      setResult({ sub, kind: "add" });
      setAddOpen(false);
      setNewName("");
      setNewUrl("");
      setNewUa("");
      setNewProfileId("");
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      await removeSubscription(id);
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleToggle = async (sub: SubscriptionView) => {
    setBusy(true);
    setError(null);
    try {
      await setSubscriptionEnabled(sub.id, !sub.enabled);
      await refreshSubs();
      // 本地刷新配置，与首页选中的生效订阅保持同步（停用当前生效订阅时后端会清除选中）。
      await loadConfig();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRefresh = async (id: string) => {
    setRefreshingId(id);
    setError(null);
    setResult(null);
    try {
      const sub = await refreshSubscription(id);
      setResult({ sub, kind: "refresh" });
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setRefreshingId(null);
    }
  };

  const openEdit = (sub: SubscriptionView) => {
    setEditSub(sub);
    setEditName(sub.name);
    setEditUrl(sub.url);
    setEditUa(sub.user_agent ?? "");
    setEditProfileId(sub.profile_id ?? "");
    setError(null);
  };

  const handleEditSave = async () => {
    if (!editSub) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const ua = editUa.trim();
      // 「不关联」传空串（与后端 `Some("")` 取消关联语义对齐；`None` 或空串均取消）。
      await updateSubscription(editSub.id, editName.trim(), editUrl.trim(), editProfileId, ua || undefined);
      setEditSub(null);
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const anyEnabled = subs.some((sub) => sub.enabled);
  const formValid = newName.trim().length > 0 && newUrl.trim().length > 0;

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">订阅</h1>
        <p className="text-sm text-muted">
          多订阅源管理：拉取节点、用量与到期展示，启用后可被首页选择，选择的订阅唯一生效
        </p>
      </div>

      <Card>
        <Card.Header>
          <Card.Title>订阅列表</Card.Title>
          <Card.Description>添加后立即拉取一次；节点数 / 用量 / 到期为最近一次拉取结果</Card.Description>
        </Card.Header>
        <Card.Content>
          {subs.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
              <span className="text-sm text-muted">暂无订阅</span>
              <span className="text-xs text-muted/80">点击「添加订阅」添加首个订阅源，启用后重启代理应用生效</span>
            </div>
          ) : (
            <Table>
              <Table.ScrollContainer>
                <Table.Content aria-label="订阅列表" className="min-w-[880px]">
                  <Table.Header>
                    <Table.Column isRowHeader>名称</Table.Column>
                    <Table.Column>URL</Table.Column>
                    <Table.Column>节点数</Table.Column>
                    <Table.Column>格式</Table.Column>
                    <Table.Column>用量</Table.Column>
                    <Table.Column>到期时间</Table.Column>
                    <Table.Column>覆写</Table.Column>
                    <Table.Column>启用</Table.Column>
                    <Table.Column>操作</Table.Column>
                  </Table.Header>
                  <Table.Body>
                    {subs.map((sub) => {
                      const percent = usagePercent(sub.userinfo);
                      const linkedProfile = sub.profile_id ? profiles.find((p) => p.id === sub.profile_id) : undefined;
                      return (
                        <Table.Row key={sub.id}>
                          <Table.Cell className="max-w-[200px] truncate">
                            <span title={sub.name}>{sub.name}</span>
                          </Table.Cell>
                          <Table.Cell className="max-w-[240px] truncate font-mono text-xs">
                            <span title={sub.url}>{sub.url}</span>
                          </Table.Cell>
                          <Table.Cell>{sub.node_count > 0 ? sub.node_count : "-"}</Table.Cell>
                          <Table.Cell>
                            <div className="flex flex-wrap items-center gap-1">
                              {sub.format ? (
                                <Chip size="sm" variant="soft" color={formatColor(sub.format)}>
                                  {formatLabel(sub.format)}
                                </Chip>
                              ) : (
                                <span className="text-muted">-</span>
                              )}
                              {sub.format === "ClashYaml" && clientCoreType === "singbox" && (
                                <span className="text-xs text-warning">需 mihomo 核心</span>
                              )}
                              {sub.format === "SingBoxJson" && clientCoreType === "mihomo" && (
                                <span className="text-xs text-warning">需 sing-box 核心</span>
                              )}
                            </div>
                          </Table.Cell>
                          <Table.Cell className="min-w-[180px]">
                            <Meter
                              aria-label={`${sub.name} 流量用量`}
                              value={percent}
                              size="sm"
                              color={usageColor(percent)}
                              valueLabel={usageText(sub.userinfo)}
                              className="w-full"
                            >
                              <Meter.Output />
                              <Meter.Track>
                                <Meter.Fill />
                              </Meter.Track>
                            </Meter>
                          </Table.Cell>
                          <Table.Cell>{formatExpire(sub.userinfo?.expire)}</Table.Cell>
                          <Table.Cell>
                            {sub.profile_id ? (
                              linkedProfile ? (
                                <span className="block max-w-[140px] truncate text-xs" title={linkedProfile.name}>
                                  {linkedProfile.name}
                                </span>
                              ) : (
                                <span className="text-xs text-warning">已失效</span>
                              )
                            ) : (
                              <span className="text-muted">-</span>
                            )}
                          </Table.Cell>
                          <Table.Cell>
                            <Switch
                              aria-label={`启用 ${sub.name}`}
                              isSelected={sub.enabled}
                              isDisabled={busy}
                              onChange={() => void handleToggle(sub)}
                            >
                              <Switch.Content>
                                <Switch.Control>
                                  <Switch.Thumb />
                                </Switch.Control>
                                <span className="sr-only">{sub.enabled ? "启用" : "停用"}</span>
                              </Switch.Content>
                            </Switch>
                          </Table.Cell>
                          <Table.Cell>
                            <div className="flex items-center gap-2">
                              <Button size="sm" variant="tertiary" isDisabled={busy} onPress={() => setPreviewSub(sub)}>
                                预览
                              </Button>
                              <Button size="sm" variant="secondary" isDisabled={busy} onPress={() => openEdit(sub)}>
                                编辑
                              </Button>
                              <Button
                                size="sm"
                                variant="secondary"
                                isDisabled={busy}
                                isPending={refreshingId === sub.id}
                                onPress={() => void handleRefresh(sub.id)}
                              >
                                刷新
                              </Button>
                              <Button
                                size="sm"
                                variant="tertiary"
                                isDisabled={busy}
                                onPress={() => void handleRemove(sub.id)}
                              >
                                删除
                              </Button>
                            </div>
                          </Table.Cell>
                        </Table.Row>
                      );
                    })}
                  </Table.Body>
                </Table.Content>
              </Table.ScrollContainer>
            </Table>
          )}
        </Card.Content>
        <Card.Footer>
          <div className="flex w-full items-center justify-between gap-3">
            <Button variant="secondary" isDisabled={busy} onPress={() => void refreshSubs()}>
              刷新列表
            </Button>
            <Button variant="primary" isDisabled={busy} onPress={() => setAddOpen(true)}>
              添加订阅
            </Button>
          </div>
        </Card.Footer>
      </Card>

      {anyEnabled && (
        <Alert status="accent">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>生效提示</Alert.Title>
            <Alert.Description>启用的订阅可在首页选择，首页选择的订阅唯一生效</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {result && (
        <Alert status={result.sub.error ? "warning" : "success"}>
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>
              {result.kind === "add"
                ? result.sub.error
                  ? "订阅已添加，但首次拉取失败"
                  : "订阅已添加"
                : result.sub.error
                  ? "刷新失败，已保留上次数据"
                  : "订阅已更新"}
            </Alert.Title>
            <Alert.Description className="break-all">
              {result.sub.error
                ? `「${result.sub.name}」${result.sub.error}`
                : `「${result.sub.name}」共 ${result.sub.node_count} 个节点`}
            </Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description className="break-all">{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      <Modal.Backdrop isOpen={addOpen} onOpenChange={setAddOpen}>
        <Modal.Container>
          <Modal.Dialog className="sm:max-w-[480px]">
            <Modal.CloseTrigger />
            <Modal.Header>
              <Modal.Heading>添加订阅</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="flex flex-col gap-4">
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-name">名称</Label>
                <Input
                  id="sub-name"
                  aria-label="订阅名称"
                  value={newName}
                  onChange={(event) => setNewName(event.target.value)}
                  placeholder="我的机场"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-url">URL</Label>
                <Input
                  id="sub-url"
                  aria-label="订阅 URL"
                  value={newUrl}
                  onChange={(event) => setNewUrl(event.target.value)}
                  placeholder="https://example.com/sub"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-ua">User-Agent（可选）</Label>
                <Input
                  id="sub-ua"
                  aria-label="订阅 User-Agent"
                  value={newUa}
                  onChange={(event) => setNewUa(event.target.value)}
                  placeholder="留空使用默认 clash.meta"
                  fullWidth
                />
                <div className="flex flex-wrap items-center gap-1.5 pt-1">
                  <span className="text-xs text-muted">常用：</span>
                  {UA_PRESETS.map((preset) => (
                    <Button
                      key={preset.value}
                      size="sm"
                      variant={newUa === preset.value ? "primary" : "secondary"}
                      onPress={() => setNewUa(preset.value)}
                    >
                      {preset.label}
                    </Button>
                  ))}
                </div>
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-profile">关联覆写</Label>
                {/* 添加时订阅尚未拉取（format 未知，无法推导核心），跟随全局核心过滤。 */}
                {coreProfiles.length > 0 ? (
                  <Select
                    id="sub-profile"
                    aria-label="关联覆写"
                    placeholder="选择覆写模板"
                    value={newProfileId}
                    onChange={(value) => setNewProfileId(String(value ?? ""))}
                    fullWidth
                  >
                    <Select.Trigger>
                      <Select.Value />
                      <Select.Indicator />
                    </Select.Trigger>
                    <Select.Popover>
                      <ListBox>
                        <ListBox.Item id="" textValue="不关联">
                          不关联
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                        {coreProfiles.map((profile) => (
                          <ListBox.Item key={profile.id} id={profile.id} textValue={profile.name}>
                            {profile.name}
                            <ListBox.ItemIndicator />
                          </ListBox.Item>
                        ))}
                      </ListBox>
                    </Select.Popover>
                  </Select>
                ) : (
                  <p className="text-xs text-warning">
                    当前核心（{coreLabel(clientCoreType)}）暂无覆写模板，可到「覆写」页创建
                  </p>
                )}
              </div>
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="secondary" onPress={() => setAddOpen(false)}>
                取消
              </Button>
              <Button variant="primary" isPending={busy} isDisabled={!formValid} onPress={() => void handleAdd()}>
                添加
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      <Modal.Backdrop
        isOpen={!!editSub}
        onOpenChange={(open) => {
          if (!open) setEditSub(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog className="sm:max-w-[480px]">
            <Modal.CloseTrigger />
            <Modal.Header>
              <Modal.Heading>编辑订阅</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="flex flex-col gap-4">
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-edit-name">名称</Label>
                <Input
                  id="sub-edit-name"
                  aria-label="订阅名称"
                  value={editName}
                  onChange={(event) => setEditName(event.target.value)}
                  placeholder="我的机场"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-edit-url">URL</Label>
                <Input
                  id="sub-edit-url"
                  aria-label="订阅 URL"
                  value={editUrl}
                  onChange={(event) => setEditUrl(event.target.value)}
                  placeholder="https://example.com/sub"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-edit-ua">User-Agent（可选）</Label>
                <Input
                  id="sub-edit-ua"
                  aria-label="订阅 User-Agent"
                  value={editUa}
                  onChange={(event) => setEditUa(event.target.value)}
                  placeholder="留空使用默认 clash.meta"
                  fullWidth
                />
                <div className="flex flex-wrap items-center gap-1.5 pt-1">
                  <span className="text-xs text-muted">常用：</span>
                  {UA_PRESETS.map((preset) => (
                    <Button
                      key={preset.value}
                      size="sm"
                      variant={editUa === preset.value ? "primary" : "secondary"}
                      onPress={() => setEditUa(preset.value)}
                    >
                      {preset.label}
                    </Button>
                  ))}
                </div>
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-edit-profile">关联覆写</Label>
                {/* 编辑时按订阅自身 format 推导的核心过滤（未拉取/ShareLinks 时跟随全局核心）。 */}
                {editCoreProfiles.length > 0 ? (
                  <Select
                    id="sub-edit-profile"
                    aria-label="关联覆写"
                    placeholder="选择覆写模板"
                    value={editProfileId}
                    onChange={(value) => setEditProfileId(String(value ?? ""))}
                    fullWidth
                  >
                    <Select.Trigger>
                      <Select.Value />
                      <Select.Indicator />
                    </Select.Trigger>
                    <Select.Popover>
                      <ListBox>
                        <ListBox.Item id="" textValue="不关联">
                          不关联
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                        {editCoreProfiles.map((profile) => (
                          <ListBox.Item key={profile.id} id={profile.id} textValue={profile.name}>
                            {profile.name}
                            <ListBox.ItemIndicator />
                          </ListBox.Item>
                        ))}
                      </ListBox>
                    </Select.Popover>
                  </Select>
                ) : (
                  <p className="text-xs text-warning">
                    当前核心（{coreLabel(editCoreType)}）暂无覆写模板，可到「覆写」页创建
                  </p>
                )}
              </div>
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="secondary" onPress={() => setEditSub(null)}>
                取消
              </Button>
              <Button
                variant="primary"
                isPending={busy}
                isDisabled={editName.trim().length === 0 || editUrl.trim().length === 0}
                onPress={() => void handleEditSave()}
              >
                保存
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      <ConfigPreviewModal
        isOpen={previewSub !== null}
        onClose={() => setPreviewSub(null)}
        title={previewSub ? `配置预览 — ${previewSub.name}` : ""}
        subscriptionId={previewSub?.id}
      />
    </div>
  );
}
