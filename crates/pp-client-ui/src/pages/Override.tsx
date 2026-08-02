import { useCallback, useEffect, useMemo, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView } from "@codemirror/view";
import { Alert, AlertDialog, Button, Card, Chip, Input, Label, ListBox, Modal, Select, Tabs } from "@heroui/react";
import clsx from "clsx";
import { createProfile, deleteProfile, getProfile, listProfiles, toErrorMessage, updateProfile } from "../api";
import type { CoreType, ProfileDetailView, ProfileView } from "../api";

const JS_PLACEHOLDER = `function main(config) {
  // 在这里修改最终生成的配置
  config.log = config.log ?? { level: "info" };
  return config;
}`;

const YAML_PLACEHOLDER = `# 按 RFC 7386 深合并：对象递归合并，数组 / 标量整体替换
# 示例：覆盖出站端口
# mixed-port: 7890
`;

const CORE_LABELS: Record<CoreType, string> = {
  singbox: "sing-box",
  mihomo: "mihomo",
};

/** 核心类型 Chip 配色：sing-box 用强调色、mihomo 用警告色区分。 */
const CORE_CHIP_COLORS: Record<CoreType, "accent" | "warning"> = {
  singbox: "accent",
  mihomo: "warning",
};

type EditorLanguage = "yaml" | "json" | "js";

interface EditorProps {
  value: string;
  onChange: (value: string) => void;
  language: EditorLanguage;
  placeholder?: string;
  height?: string;
  readOnly?: boolean;
}

/** CodeMirror 编辑器包装：固定深色主题（One Dark），支持 YAML / JSON / JS。 */
function Editor({ value, onChange, language, placeholder, height = "320px", readOnly = false }: EditorProps) {
  const extensions = useMemo(() => {
    const base = [EditorView.lineWrapping];
    switch (language) {
      case "yaml":
        return [...base, yaml()];
      case "json":
        return [...base, json()];
      default:
        return [...base, javascript()];
    }
  }, [language]);

  return (
    <CodeMirror
      value={value}
      height={height}
      extensions={extensions}
      theme={oneDark}
      placeholder={placeholder}
      onChange={onChange}
      editable={!readOnly}
      className="overflow-hidden rounded-md border border-border text-sm font-mono"
      basicSetup={{
        lineNumbers: true,
        highlightActiveLineGutter: true,
        highlightActiveLine: true,
        foldGutter: true,
        bracketMatching: true,
        closeBrackets: true,
        autocompletion: true,
        tabSize: 2,
      }}
    />
  );
}

export default function Override() {
  const [profiles, setProfiles] = useState<ProfileView[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<ProfileDetailView | null>(null);
  const [yamlValue, setYamlValue] = useState("");
  const [jsValue, setJsValue] = useState("");
  const [yamlUrl, setYamlUrl] = useState("");
  const [jsUrl, setJsUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // 新建模板对话框
  const [createOpen, setCreateOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newCoreType, setNewCoreType] = useState<CoreType>("singbox");

  // 删除确认
  const [deleteTarget, setDeleteTarget] = useState<ProfileView | null>(null);

  const selectedProfile = profiles.find((profile) => profile.id === selectedId) ?? null;

  const refreshList = useCallback(async () => {
    try {
      setProfiles(await listProfiles());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refreshList();
  }, [refreshList]);

  const selectProfile = useCallback(async (id: string) => {
    setSelectedId(id);
    setError(null);
    setSuccess(null);
    try {
      const profile = await getProfile(id);
      setDetail(profile);
      setYamlValue(profile.yaml_override);
      setJsValue(profile.js_override);
      setYamlUrl(profile.yaml_url ?? "");
      setJsUrl(profile.js_url ?? "");
    } catch (err) {
      setDetail(null);
      setError(toErrorMessage(err));
    }
  }, []);

  const handleSave = async () => {
    if (!detail) return;
    setBusy(true);
    setError(null);
    setSuccess(null);
    try {
      await updateProfile({
        id: detail.id,
        name: detail.name,
        yaml_override: yamlValue,
        js_override: jsValue,
        yaml_url: yamlUrl,
        js_url: jsUrl,
      });
      setSuccess(`模板「${detail.name}」已保存，需重启代理后生效`);
      await refreshList();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleCreate = async () => {
    setBusy(true);
    setError(null);
    setSuccess(null);
    try {
      const created = await createProfile({ name: newName.trim(), core_type: newCoreType });
      setSuccess(`模板「${created.name}」已创建`);
      setCreateOpen(false);
      setNewName("");
      setNewCoreType("singbox");
      await refreshList();
      void selectProfile(created.id);
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    setBusy(true);
    setError(null);
    setSuccess(null);
    try {
      await deleteProfile(deleteTarget.id);
      setSuccess(`模板「${deleteTarget.name}」已删除`);
      if (selectedId === deleteTarget.id) {
        setSelectedId(null);
        setDetail(null);
      }
      setDeleteTarget(null);
      await refreshList();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">覆写</h1>
        <p className="text-sm text-muted">
          多模板管理：按核心类型维护覆写模板，在订阅页关联后随订阅生效；合成配置预览已移至首页与订阅页
        </p>
      </div>

      <div className="flex flex-col gap-6 lg:flex-row lg:items-start">
        {/* 左侧：模板列表 */}
        <Card className="w-full shrink-0 lg:w-80">
          <Card.Header>
            <Card.Title>覆写模板</Card.Title>
            <Card.Description>按核心类型维护，在订阅页关联后随订阅生效</Card.Description>
          </Card.Header>
          <Card.Content className="flex flex-col gap-2">
            {profiles.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                <span className="text-sm text-muted">暂无模板</span>
                <span className="text-xs text-muted/80">点击下方「新建模板」创建首个覆写模板</span>
              </div>
            ) : (
              profiles.map((profile) => {
                const isSelected = profile.id === selectedId;
                return (
                  <div
                    key={profile.id}
                    className={clsx(
                      "flex items-center gap-3 rounded-lg border p-3 transition-colors",
                      isSelected ? "border-accent/60 bg-accent/5" : "border-border/70 bg-surface-secondary/40",
                    )}
                  >
                    <button
                      type="button"
                      onClick={() => void selectProfile(profile.id)}
                      className="flex min-w-0 flex-1 flex-col items-start gap-1 text-left"
                    >
                      <span
                        className={clsx(
                          "truncate text-sm",
                          isSelected ? "font-medium text-foreground" : "text-foreground/90",
                        )}
                      >
                        {profile.name}
                      </span>
                      <span className="flex items-center gap-2">
                        <Chip size="sm" variant="soft" color={CORE_CHIP_COLORS[profile.core_type]}>
                          {CORE_LABELS[profile.core_type]}
                        </Chip>
                      </span>
                    </button>
                    <div className="flex shrink-0 flex-col gap-1">
                      <Button
                        size="sm"
                        variant="secondary"
                        isDisabled={busy}
                        onPress={() => void selectProfile(profile.id)}
                      >
                        编辑
                      </Button>
                      <Button size="sm" variant="tertiary" isDisabled={busy} onPress={() => setDeleteTarget(profile)}>
                        删除
                      </Button>
                    </div>
                  </div>
                );
              })
            )}
          </Card.Content>
          <Card.Footer>
            <Button variant="primary" fullWidth isDisabled={busy} onPress={() => setCreateOpen(true)}>
              新建模板
            </Button>
          </Card.Footer>
        </Card>

        {/* 右侧：编辑器区 */}
        {selectedProfile ? (
          <Card key={selectedProfile.id} className="min-w-0 flex-1">
            <Card.Header>
              <div className="flex flex-wrap items-center gap-2">
                <Card.Title>{selectedProfile.name}</Card.Title>
                <Chip size="sm" variant="soft" color={CORE_CHIP_COLORS[selectedProfile.core_type]}>
                  {CORE_LABELS[selectedProfile.core_type]}
                </Chip>
              </div>
              <Card.Description>
                针对该模板独立维护本地 YAML / JS 覆写与远程 URL（远程为基底、本地叠加），保存后需重启代理生效
              </Card.Description>
            </Card.Header>
            <Card.Content className="flex flex-col gap-4">
              <Tabs>
                <Tabs.ListContainer>
                  <Tabs.List aria-label="覆写编辑器">
                    <Tabs.Tab id="yaml">
                      YAML 覆写
                      <Tabs.Indicator />
                    </Tabs.Tab>
                    <Tabs.Tab id="js">
                      JS 覆写
                      <Tabs.Indicator />
                    </Tabs.Tab>
                  </Tabs.List>
                </Tabs.ListContainer>
                <Tabs.Panel id="yaml" className="flex flex-col gap-2 pt-3">
                  <div className="flex flex-col gap-1">
                    <Label htmlFor="yaml-remote-url">远程 URL（可选）</Label>
                    <Input
                      id="yaml-remote-url"
                      aria-label="YAML 覆写远程 URL"
                      value={yamlUrl}
                      onChange={(event) => setYamlUrl(event.target.value)}
                      placeholder="https://example.com/remote-override.yaml"
                      fullWidth
                    />
                    <p className="text-xs text-muted">
                      启动时拉取远程 YAML 作为基底，本地 YAML 深合并覆盖（远程失效自动回退缓存）
                    </p>
                  </div>
                  <Editor value={yamlValue} onChange={setYamlValue} language="yaml" placeholder={YAML_PLACEHOLDER} />
                  <p className="text-xs text-muted">
                    {selectedProfile.core_type === "mihomo"
                      ? "mihomo 推荐优先使用 YAML 覆写做深合并覆盖；留空表示不启用。"
                      : "按 RFC 7386 深合并：对象递归合并，数组与标量整体替换；留空表示不启用。"}
                  </p>
                </Tabs.Panel>
                <Tabs.Panel id="js" className="flex flex-col gap-2 pt-3">
                  <div className="flex flex-col gap-1">
                    <Label htmlFor="js-remote-url">远程 URL（可选）</Label>
                    <Input
                      id="js-remote-url"
                      aria-label="JS 覆写远程 URL"
                      value={jsUrl}
                      onChange={(event) => setJsUrl(event.target.value)}
                      placeholder="https://example.com/remote-override.js"
                      fullWidth
                    />
                    <p className="text-xs text-muted">
                      启动时拉取远程 JS 覆写，远程 main 先执行、本地 main
                      后执行（本地可见远程结果；远程失效自动回退缓存）
                    </p>
                  </div>
                  <Editor value={jsValue} onChange={setJsValue} language="js" placeholder={JS_PLACEHOLDER} />
                  <p className="text-xs text-muted">
                    {selectedProfile.core_type === "singbox"
                      ? "sing-box 推荐优先使用 JS 覆写做程序化调整；需定义 function main(config) 并返回 config；留空表示不启用。"
                      : "双核心通用：需定义 function main(config) 并返回 config；留空表示不启用。"}
                  </p>
                </Tabs.Panel>
              </Tabs>

              <Alert status="accent">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>配置生成链路</Alert.Title>
                  <Alert.Description>
                    订阅取节点 → 内置模板 → 远程 YAML → 本地 YAML → 远程 JS → 本地 JS →
                    核心。远程为基底、本地叠加覆盖；远程失效自动回退缓存。覆写修改需重启代理后生效。
                  </Alert.Description>
                </Alert.Content>
              </Alert>
            </Card.Content>
            <Card.Footer>
              <div className="flex w-full items-center justify-end gap-3">
                <Button variant="primary" isPending={busy} onPress={() => void handleSave()}>
                  保存
                </Button>
              </div>
            </Card.Footer>
          </Card>
        ) : (
          <Card className="flex min-h-[420px] flex-1 items-center justify-center">
            <Card.Content className="flex flex-col items-center justify-center gap-2 py-16 text-center">
              <span className="text-sm text-muted">选择一个模板开始编辑</span>
              <span className="text-xs text-muted/80">从左侧列表选择模板，或点击「新建模板」创建</span>
            </Card.Content>
          </Card>
        )}
      </div>

      {success && (
        <Alert status="success">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作成功</Alert.Title>
            <Alert.Description>{success}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {/* 新建模板 */}
      <Modal.Backdrop isOpen={createOpen} onOpenChange={setCreateOpen}>
        <Modal.Container>
          <Modal.Dialog className="sm:max-w-[480px]">
            <Modal.CloseTrigger />
            <Modal.Header>
              <Modal.Heading>新建覆写模板</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="flex flex-col gap-4">
              <div className="flex flex-col gap-1">
                <Label htmlFor="profile-name">名称</Label>
                <Input
                  id="profile-name"
                  aria-label="模板名称"
                  value={newName}
                  onChange={(event) => setNewName(event.target.value)}
                  placeholder="例如：香港-去广告"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label>核心类型</Label>
                <Select
                  aria-label="核心类型"
                  placeholder="选择核心类型"
                  value={newCoreType}
                  onChange={(value) => setNewCoreType((value as CoreType | null) ?? "singbox")}
                  fullWidth
                >
                  <Select.Trigger>
                    <Select.Value />
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox>
                      <ListBox.Item id="singbox" textValue="sing-box">
                        sing-box
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                      <ListBox.Item id="mihomo" textValue="mihomo">
                        mihomo
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    </ListBox>
                  </Select.Popover>
                </Select>
              </div>
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="secondary" onPress={() => setCreateOpen(false)}>
                取消
              </Button>
              <Button
                variant="primary"
                isPending={busy}
                isDisabled={newName.trim().length === 0}
                onPress={() => void handleCreate()}
              >
                创建
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      {/* 删除确认 */}
      <AlertDialog.Backdrop
        isOpen={deleteTarget !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>删除模板</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p>确定删除模板「{deleteTarget?.name}」吗？该操作不可撤销。</p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setDeleteTarget(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" isPending={busy} onPress={() => void handleDelete()}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}
