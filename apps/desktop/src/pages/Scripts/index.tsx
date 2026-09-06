import { useQuery } from "@tanstack/react-query";
import { Alert, Tabs } from "@heroui/react";
import { listRemotes, listTasks, getRemoteIcon, toErrorMessage } from "@pp/client-core";
import { REMOTES_KEY, TASKS_KEY } from "@pp/client-core";
import type { RemoteResource } from "@pp/client-core";
import { useCapabilities } from "@pp/client-core";
import { MobileBackHeader } from "../../layout/mobile/MobileBackHeader";
import RemotesTab from "./RemotesTab";
import TasksTab from "./TasksTab";
import ImportTab from "./ImportTab";

async function fetchRemotesWithIcons(): Promise<{ remotes: RemoteResource[]; iconCache: Record<string, string> }> {
  const list = await listRemotes();
  const icons: Record<string, string> = {};
  await Promise.allSettled(
    list
      .filter((r) => r.icon)
      .map(async (r) => {
        const dataUrl = await getRemoteIcon(r.name);
        if (dataUrl) icons[r.name] = dataUrl;
      }),
  );
  return { remotes: list, iconCache: icons };
}

export default function Scripts() {
  const { data: capabilities } = useCapabilities();
  const capScriptsRemote = capabilities?.capabilities.scripts_remote ?? true;
  const capCronTasks = capabilities?.capabilities.cron_tasks ?? true;

  const {
    data: remotesData,
    isLoading: remotesLoading,
    error: remotesError,
  } = useQuery({
    queryKey: REMOTES_KEY,
    queryFn: fetchRemotesWithIcons,
    enabled: capScriptsRemote,
    refetchInterval: 5000,
  });

  const {
    data: tasks,
    isLoading: tasksLoading,
    error: tasksError,
  } = useQuery({
    queryKey: TASKS_KEY,
    queryFn: listTasks,
    enabled: capCronTasks,
    refetchInterval: 5000,
  });

  const error = remotesError ? toErrorMessage(remotesError) : tasksError ? toErrorMessage(tasksError) : null;

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
              remotes={remotesData?.remotes ?? []}
              iconCache={remotesData?.iconCache ?? {}}
              isLoading={remotesLoading}
              error={error}
            />
          </Tabs.Panel>
        )}

        {capCronTasks && (
          <Tabs.Panel className="flex flex-col gap-4 pt-4" id="tasks">
            <TasksTab tasks={tasks ?? []} isLoading={tasksLoading} error={error} />
          </Tabs.Panel>
        )}

        {capScriptsRemote && (
          <Tabs.Panel className="flex flex-col gap-4 pt-4" id="import">
            <ImportTab />
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
