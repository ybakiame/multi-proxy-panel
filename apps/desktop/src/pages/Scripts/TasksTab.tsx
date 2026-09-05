import { useState } from "react";
import { Alert, Button, Card, Table } from "@heroui/react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { runTask, toErrorMessage } from "../../api";
import { TASKS_KEY } from "../../api/keys";
import type { TaskScriptView } from "../../api";
import { formatTime } from "./utils";

interface TasksTabProps {
  tasks: TaskScriptView[];
  isLoading: boolean;
  error: string | null;
}

export default function TasksTab({ tasks, isLoading, error }: TasksTabProps) {
  const queryClient = useQueryClient();
  const [runResult, setRunResult] = useState<{ name: string; output: string } | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [runningTask, setRunningTask] = useState<string | null>(null);

  const runMutation = useMutation({
    mutationFn: ({ name }: { name: string }) => runTask(name),
    onSuccess: (output, { name }) => {
      setRunResult({ name, output });
      setActionError(null);
      void queryClient.invalidateQueries({ queryKey: TASKS_KEY });
    },
    onError: (err) => {
      setActionError(toErrorMessage(err));
      setRunResult(null);
    },
    onSettled: () => setRunningTask(null),
  });

  const handleRunTask = (name: string) => {
    setRunningTask(name);
    setActionError(null);
    setRunResult(null);
    runMutation.mutate({ name });
  };

  const displayError = error ?? actionError;

  return (
    <div className="flex flex-col gap-4">
      <Card>
        <Card.Header>
          <Card.Title>定时任务</Card.Title>
          <Card.Description>远程订阅中的 cron 任务脚本，阶段③解耦后不再依赖 MITM</Card.Description>
        </Card.Header>
        <Card.Content>
          {isLoading && tasks.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
              <span className="text-sm text-muted">正在加载定时任务…</span>
            </div>
          ) : tasks.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
              <span className="text-sm text-muted">暂无定时任务</span>
              <span className="text-xs text-muted/80">远程资源中的 [task_local] / cron 脚本会在此列出</span>
            </div>
          ) : (
            <Table>
              <Table.ScrollContainer>
                <Table.Content aria-label="定时任务" className="min-w-[720px]">
                  <Table.Header>
                    <Table.Column isRowHeader>名称</Table.Column>
                    <Table.Column>cron</Table.Column>
                    <Table.Column>下次执行</Table.Column>
                    <Table.Column>上次执行</Table.Column>
                    <Table.Column>上次错误</Table.Column>
                    <Table.Column>操作</Table.Column>
                  </Table.Header>
                  <Table.Body>
                    {tasks.map((task) => (
                      <Table.Row key={task.name}>
                        <Table.Cell className="max-w-[180px] truncate">
                          <span title={task.name}>{task.name}</span>
                        </Table.Cell>
                        <Table.Cell className="font-mono text-xs">{task.cron_expr}</Table.Cell>
                        <Table.Cell>{formatTime(task.next_run)}</Table.Cell>
                        <Table.Cell>{formatTime(task.last_run)}</Table.Cell>
                        <Table.Cell className="max-w-[200px] truncate">
                          <span title={task.last_error ?? "-"}>{task.last_error ?? "-"}</span>
                        </Table.Cell>
                        <Table.Cell>
                          <Button
                            size="sm"
                            variant="secondary"
                            isPending={runningTask === task.name}
                            isDisabled={runningTask !== null}
                            onPress={() => void handleRunTask(task.name)}
                          >
                            运行
                          </Button>
                        </Table.Cell>
                      </Table.Row>
                    ))}
                  </Table.Body>
                </Table.Content>
              </Table.ScrollContainer>
            </Table>
          )}
        </Card.Content>
        <Card.Footer>
          <Button
            variant="secondary"
            isDisabled={isLoading && tasks.length === 0}
            onPress={() => void queryClient.invalidateQueries({ queryKey: TASKS_KEY })}
          >
            刷新
          </Button>
        </Card.Footer>
      </Card>

      {runResult && (
        <Alert status="success">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>任务「{runResult.name}」已运行</Alert.Title>
            <Alert.Description className="break-all font-mono text-xs">
              {runResult.output || "$done()"}
            </Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {displayError && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description>{displayError}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}
    </div>
  );
}
