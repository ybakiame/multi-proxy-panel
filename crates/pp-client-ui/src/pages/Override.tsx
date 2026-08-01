import { useCallback, useEffect, useMemo, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView } from "@codemirror/view";
import { Alert, Button, Card } from "@heroui/react";
import { getProfileOverrides, previewCoreConfig, saveProfileOverrides, toErrorMessage } from "../api";
import type { ProfileOverrides } from "../api";
import { useAppStore } from "../store";

const JS_PLACEHOLDER = `function main(config) {
  // 在这里修改最终生成的配置
  config.log = config.log ?? { level: "info" };
  return config;
}`;

const YAML_PLACEHOLDER = `# 按 RFC 7386 深合并：对象递归合并，数组 / 标量整体替换
# 示例：覆盖出站端口
# mixed-port: 7890
`;

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
  const coreType = useAppStore((state) => state.config?.core_type);
  const [yamlValue, setYamlValue] = useState("");
  const [jsValue, setJsValue] = useState("");
  const [preview, setPreview] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const overrides = await getProfileOverrides();
      setYamlValue(overrides.yaml_override);
      setJsValue(overrides.js_override);
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleSave = async () => {
    setBusy(true);
    setError(null);
    setSuccess(null);
    const overrides: ProfileOverrides = { yaml_override: yamlValue, js_override: jsValue };
    try {
      await saveProfileOverrides(overrides);
      setSuccess("复写已保存，需重启代理后生效");
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePreview = async () => {
    setBusy(true);
    setError(null);
    setSuccess(null);
    try {
      setPreview(await previewCoreConfig());
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const previewLanguage: EditorLanguage = coreType === "mihomo" ? "yaml" : "json";

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">复写</h1>
        <p className="text-sm text-muted">对生成的核心配置做 YAML 深合并与 JS 脚本复写</p>
      </div>

      <Alert status="accent">
        <Alert.Indicator />
        <Alert.Content>
          <Alert.Title>配置生成链路</Alert.Title>
          <Alert.Description>
            订阅取节点 → 内置模板 → YAML 复写 → JS 复写 → 核心。订阅内容只用于提取代理节点，
            实际运行配置由本地模板生成，复写在此基础上生效。
          </Alert.Description>
        </Alert.Content>
      </Alert>

      <Card>
        <Card.Header>
          <Card.Title>YAML 复写</Card.Title>
          <Card.Description>按 RFC 7386 深合并：对象递归合并，数组与标量整体替换。留空表示不启用。</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <Editor value={yamlValue} onChange={setYamlValue} language="yaml" placeholder={YAML_PLACEHOLDER} />
        </Card.Content>
        <Card.Footer>
          <Button variant="primary" isPending={busy} onPress={() => void handleSave()}>
            保存
          </Button>
        </Card.Footer>
      </Card>

      <Card>
        <Card.Header>
          <Card.Title>JS 复写</Card.Title>
          <Card.Description>
            同步纯函数 <code className="font-mono text-xs">function main(config) {"{ ... return config; }"}</code>， 在
            YAML 复写之后执行。留空表示不启用。
          </Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <Editor value={jsValue} onChange={setJsValue} language="js" placeholder={JS_PLACEHOLDER} />
        </Card.Content>
        <Card.Footer>
          <Button variant="primary" isPending={busy} onPress={() => void handleSave()}>
            保存
          </Button>
        </Card.Footer>
      </Card>

      <Card>
        <Card.Header>
          <Card.Title>生效配置预览</Card.Title>
          <Card.Description>
            按当前保存的复写拉取订阅并生成最终核心配置（不含 MITM 链路）。若尚未保存复写，预览使用空复写。
          </Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          {preview === null ? (
            <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
              <span className="text-sm text-muted">尚未生成预览</span>
              <span className="text-xs text-muted/80">点击「生成预览」查看最终核心配置（只读）</span>
            </div>
          ) : (
            <Editor value={preview} onChange={() => undefined} language={previewLanguage} height="480px" readOnly />
          )}
        </Card.Content>
        <Card.Footer>
          <Button variant="secondary" isPending={busy} onPress={() => void handlePreview()}>
            生成预览
          </Button>
        </Card.Footer>
      </Card>

      {success && (
        <Alert status="success">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>保存成功</Alert.Title>
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
    </div>
  );
}
