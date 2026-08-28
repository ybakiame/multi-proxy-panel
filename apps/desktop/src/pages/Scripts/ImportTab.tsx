import { useState } from "react";
import { Alert, Button, Card, Label, ListBox, Select, TextArea } from "@heroui/react";
import { importConfig, toErrorMessage } from "../../api";
import type { ImportSummary } from "../../api";
import { IMPORT_DIALECT_OPTIONS } from "./utils";

interface ImportTabProps {
  busy: boolean;
  setBusy: React.Dispatch<React.SetStateAction<boolean>>;
  error: string | null;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
}

export default function ImportTab({ busy, setBusy, setError }: ImportTabProps) {
  const [importText, setImportText] = useState("");
  const [importDialect, setImportDialect] = useState<string>("loon");
  const [importResult, setImportResult] = useState<ImportSummary | null>(null);

  const handleImport = async () => {
    setBusy(true);
    setError(null);
    setImportResult(null);
    try {
      setImportResult(await importConfig(importText, importDialect));
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <Card>
        <Card.Header>
          <Card.Title>导入配置</Card.Title>
          <Card.Description>粘贴 Surge / Loon 的 rewrite / script / mitm 片段，合并进本地缓存</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <Select
            className="w-full sm:max-w-[240px]"
            placeholder="选择方言"
            value={importDialect}
            onChange={(value) => setImportDialect(String(value ?? ""))}
          >
            <Label>方言</Label>
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {IMPORT_DIALECT_OPTIONS.map((option) => (
                  <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                    {option.label}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
          <TextArea
            aria-label="配置片段"
            value={importText}
            onChange={(event) => setImportText(event.target.value)}
            placeholder={
              "[rewrite_local]\n^https?://example\\.com/api/ url-and-header https://cdn.example.com/$1\n\n[mitm]\nhostname = *.example.com"
            }
            rows={12}
            fullWidth
          />
        </Card.Content>
        <Card.Footer>
          <Button
            variant="primary"
            isPending={busy}
            isDisabled={importText.trim().length === 0}
            onPress={() => void handleImport()}
          >
            导入
          </Button>
        </Card.Footer>
      </Card>

      {importResult && (
        <Alert status={importResult.warnings.length > 0 ? "warning" : "success"}>
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>导入完成</Alert.Title>
            <Alert.Description>
              重写 {importResult.rewrites}、脚本 {importResult.scripts}、任务 {importResult.tasks}、 主机名{" "}
              {importResult.hostnames}
              {importResult.warnings.length > 0 && `，警告 ${importResult.warnings.length} 条`}
            </Alert.Description>
            {importResult.meta?.name && (
              <div className="mt-2 break-words text-sm">
                识别为：{importResult.meta.name}
                {importResult.meta.desc ? ` — ${importResult.meta.desc}` : ""}
              </div>
            )}
            {importResult.warnings.length > 0 && (
              <ul className="mt-2 list-inside list-disc space-y-1 break-words text-sm">
                {importResult.warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
            )}
          </Alert.Content>
        </Alert>
      )}
    </div>
  );
}
