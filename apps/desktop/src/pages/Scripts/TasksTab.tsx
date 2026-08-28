import { useCallback } from "react";
import { Alert, Button, Card, Table } from "@heroui/react";
import { listTasks, runTask, toErrorMessage } from "../../api";
import type { TaskScriptView } from "../../api";
import { formatTime } from "./utils";

interface TasksTabProps {
  tasks: TaskScriptView[];
  setTasks: React.Dispatch<React.SetStateAction<TaskScriptView[]>>;
  busy: boolean;
  setBusy: React.Dispatch<React.SetStateAction<boolean>>;
  error: string | null;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  runResult: { name: string; output: string } | null;
  setRunResult: React.Dispatch<React.SetStateAction<{ name: string; output: string } | null>>;
}

export default function TasksTab({ tasks, setTasks, busy, setBusy, setError, runResult, setRunResult }: TasksTabProps) {
  const refreshTasks = useCallback(async () => {
    try {
      setTasks(await listTasks());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, [setTasks, setError]);

  const handleRunTask = async (name: string) => {
    setBusy(true);
    setError(null);
    setRunResult(null);
    try {
      const output = await runTask(name);
      setRunResult({ name, output });
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
          <Card.Title>定时任务</Card.Title>
          <Card.Description>远程订阅中的 cron 任务脚本，阶段③解耦后不再依赖 MITM</Card.Description>
        </Card.Header>
        <Card.Content>
          {tasks.length === 0 ? (
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
                            isDisabled={busy}
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
          <Button variant="secondary" isDisabled={busy} onPress={() => void refreshTasks()}>
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
    </div>
  );
}
