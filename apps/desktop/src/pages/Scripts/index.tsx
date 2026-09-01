import { useCallback, useEffect, useState } from "react";
import { Alert, Tabs } from "@heroui/react";
import { listRemotes, listTasks, getRemoteIcon, toErrorMessage } from "../../api";
import type { RemoteResource, TaskScriptView } from "../../api";
import { useCapabilities } from "../../hooks/useCapabilities";
import { MobileBackHeader } from "../../layout/MobileBackHeader";
import RemotesTab from "./RemotesTab";
import TasksTab from "./TasksTab";
import ImportTab from "./ImportTab";

export default function Scripts() {
  const { data: capabilities } = useCapabilities();
  const capScriptsRemote = capabilities?.capabilities.scripts_remote ?? true;
  const capCronTasks = capabilities?.capabilities.cron_tasks ?? true;

  const [remotes, setRemotes] = useState<RemoteResource[]>([]);
  const [iconCache, setIconCache] = useState<Record<string, string>>({});
  const [tasks, setTasks] = useState<TaskScriptView[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fetchResult, setFetchResult] = useState<{
    fetched: number;
    scripts: number;
    rewrites: number;
    tasks: number;
    warnings: string[];
  } | null>(null);
  const [runResult, setRunResult] = useState<{ name: string; output: string } | null>(null);

  const refreshRemotes = useCallback(async () => {
    try {
      const list = await listRemotes();
      setRemotes(list);
      const icons: Record<string, string> = {};
      await Promise.allSettled(
        list
          .filter((r) => r.icon)
          .map(async (r) => {
            const dataUrl = await getRemoteIcon(r.name);
            if (dataUrl) icons[r.name] = dataUrl;
          }),
      );
      setIconCache(icons);
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  const refreshTasks = useCallback(async () => {
    try {
      setTasks(await listTasks());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    if (capScriptsRemote) {
      void refreshRemotes();
    }
    if (capCronTasks) {
      void refreshTasks();
    }
  }, [refreshRemotes, refreshTasks, capScriptsRemote, capCronTasks]);

  return (
    <div className="flex flex-col gap-6">
      <MobileBackHeader title="脚本" />
      <div>
        <h1 className="text-xl font-semibold">脚本</h1>
        <p className="text-sm text-muted">远程脚本 / 配置片段订阅、定时任务调度与三方配置导入</p>
      </div>

      <Tabs>
        <Tabs.ListContainer>
          <Tabs.List aria-label="脚本管理">
            {capScriptsRemote && (
              <Tabs.Tab id="remotes">
                远程资源
                <Tabs.Indicator />
              </Tabs.Tab>
            )}
            {capCronTasks && (
              <Tabs.Tab id="tasks">
                定时任务
                <Tabs.Indicator />
              </Tabs.Tab>
            )}
            {capScriptsRemote && (
              <Tabs.Tab id="import">
                配置导入
                <Tabs.Indicator />
              </Tabs.Tab>
            )}
          </Tabs.List>
        </Tabs.ListContainer>

        {capScriptsRemote && (
          <Tabs.Panel className="flex flex-col gap-4 pt-4" id="remotes">
            <RemotesTab
              remotes={remotes}
              setRemotes={setRemotes}
              iconCache={iconCache}
              setIconCache={setIconCache}
              busy={busy}
              setBusy={setBusy}
              error={error}
              setError={setError}
              fetchResult={fetchResult}
              setFetchResult={setFetchResult}
            />
          </Tabs.Panel>
        )}

        {capCronTasks && (
          <Tabs.Panel className="flex flex-col gap-4 pt-4" id="tasks">
            <TasksTab
              tasks={tasks}
              setTasks={setTasks}
              busy={busy}
              setBusy={setBusy}
              error={error}
              setError={setError}
              runResult={runResult}
              setRunResult={setRunResult}
            />
          </Tabs.Panel>
        )}

        {capScriptsRemote && (
          <Tabs.Panel className="flex flex-col gap-4 pt-4" id="import">
            <ImportTab busy={busy} setBusy={setBusy} error={error} setError={setError} />
          </Tabs.Panel>
        )}
      </Tabs>

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
