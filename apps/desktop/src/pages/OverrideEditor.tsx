import { useMemo, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView } from "@codemirror/view";
import { Alert, Button, Card, Chip, Input, Label, Tabs } from "@heroui/react";
import type { CoreType, ProfileDetailView, ProfileView } from "../api";

export const JS_PLACEHOLDER = `function main(config) {
  // 在这里修改最终生成的配置
  config.log = config.log ?? { level: "info" };
  return config;
}`;

export const YAML_PLACEHOLDER = `# 按 RFC 7386 深合并：对象递归合并，数组 / 标量整体替换
# 示例：覆盖出站端口
# mixed-port: 7890
`;

export const CORE_LABELS: Record<CoreType, string> = {
  singbox: "sing-box",
  mihomo: "mihomo",
};

/** 核心类型 Chip 配色：sing-box 用强调色、mihomo 用警告色区分。 */
export const CORE_CHIP_COLORS: Record<CoreType, "accent" | "warning"> = {
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

interface ProfileEditorProps {
  profile: ProfileView;
  detail: ProfileDetailView;
  onSave: (input: {
    id: string;
    name: string;
    yaml_override: string;
    js_override: string;
    yaml_url: string;
    js_url: string;
  }) => void;
  busy: boolean;
}

/** 独立的模板编辑器面板，key={profile.id} 确保切换模板时重新挂载、状态重置。 */
export function ProfileEditor({ profile, detail, onSave, busy }: ProfileEditorProps) {
  const [yamlValue, setYamlValue] = useState(detail.yaml_override);
  const [jsValue, setJsValue] = useState(detail.js_override);
  const [yamlUrl, setYamlUrl] = useState(detail.yaml_url ?? "");
  const [jsUrl, setJsUrl] = useState(detail.js_url ?? "");

  return (
    <Card key={profile.id} className="min-w-0 flex-1">
      <Card.Header>
        <div className="flex flex-wrap items-center gap-2">
          <Card.Title className="min-w-0 break-words">{profile.name}</Card.Title>
          <Chip size="sm" variant="soft" color={CORE_CHIP_COLORS[profile.core_type]}>
            {CORE_LABELS[profile.core_type]}
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
              {profile.core_type === "mihomo"
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
                启动时拉取远程 JS 覆写，远程 main 先执行、本地 main 后执行（本地可见远程结果；远程失效自动回退缓存）
              </p>
            </div>
            <Editor value={jsValue} onChange={setJsValue} language="js" placeholder={JS_PLACEHOLDER} />
            <p className="text-xs text-muted">
              {profile.core_type === "singbox"
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
          <Button
            variant="primary"
            isPending={busy}
            onPress={() =>
              void onSave({
                id: detail.id,
                name: detail.name,
                yaml_override: yamlValue,
                js_override: jsValue,
                yaml_url: yamlUrl,
                js_url: jsUrl,
              })
            }
          >
            保存
          </Button>
        </div>
      </Card.Footer>
    </Card>
  );
}
