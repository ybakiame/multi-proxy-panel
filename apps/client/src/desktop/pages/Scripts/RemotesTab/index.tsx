/**
 * RemotesTab page component — composition of table, modals, and fetch result.
 */

import { useState } from "react";
import { Alert, Button, Card } from "@heroui/react";
import { useQueryClient, useMutation } from "@tanstack/react-query";
import { addRemote, fetchRemotes, removeRemote, updateRemote, toErrorMessage } from "@pp/client-core";
import type { FetchReport, RemoteResource } from "@pp/client-core";
import { REMOTES_KEY } from "@pp/client-core";
import RemoteFormModal from "./RemoteFormModal";
import RemoteTable from "./RemoteTable";

interface RemotesTabProps {
  remotes: RemoteResource[];
  iconCache: Record<string, string>;
  isLoading: boolean;
  error: string | null;
}

export default function RemotesTab({ remotes, iconCache, isLoading, error }: RemotesTabProps) {
  const queryClient = useQueryClient();
  const [addOpen, setAddOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [editRemote, setEditRemote] = useState<RemoteResource | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [fetchResult, setFetchResult] = useState<FetchReport | null>(null);
  const [busy, setBusy] = useState(false);

  const invalidateRemotes = () => {
    void queryClient.invalidateQueries({ queryKey: REMOTES_KEY });
  };

  const addMutation = useMutation({
    mutationFn: addRemote,
    onSuccess: () => {
      setAddOpen(false);
      invalidateRemotes();
    },
    onError: (err) => setActionError(toErrorMessage(err)),
  });

  const removeMutation = useMutation({
    mutationFn: removeRemote,
    onSuccess: invalidateRemotes,
    onError: (err) => setActionError(toErrorMessage(err)),
  });

  const updateMutation = useMutation({
    mutationFn: updateRemote,
    onSuccess: () => {
      setEditOpen(false);
      setEditRemote(null);
      invalidateRemotes();
    },
    onError: (err) => setActionError(toErrorMessage(err)),
  });

  const fetchMutation = useMutation({
    mutationFn: fetchRemotes,
    onSuccess: (result) => {
      setFetchResult(result);
      invalidateRemotes();
    },
    onError: (err) => setActionError(toErrorMessage(err)),
  });

  const handleToggle = async (remote: RemoteResource) => {
    const next = { ...remote, enabled: !remote.enabled };
    setBusy(true);
    setActionError(null);
    try {
      await removeRemote(remote.name);
      await addRemote(next);
      invalidateRemotes();
    } catch (err) {
      setActionError(toErrorMessage(err));
      invalidateRemotes();
    }
    setBusy(false);
  };

  const handleOpenEdit = (remote: RemoteResource) => {
    setEditRemote(remote);
    setActionError(null);
    setEditOpen(true);
  };

  const displayError = error ?? actionError;
  const isBusy =
    busy || addMutation.isPending || removeMutation.isPending || updateMutation.isPending || fetchMutation.isPending;

  return (
    <div className="flex flex-col gap-4">
      <Card>
        <Card.Header>
          <Card.Title>远程资源</Card.Title>
          <Card.Description>脚本 / 配置片段订阅，按间隔拉取并落盘缓存</Card.Description>
        </Card.Header>
        <Card.Content>
          {isLoading && remotes.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
              <span className="text-sm text-muted">正在加载远程资源…</span>
            </div>
          ) : (
            <RemoteTable
              remotes={remotes}
              iconCache={iconCache}
              busy={isBusy}
              onToggle={handleToggle}
              onEdit={handleOpenEdit}
              onRemove={(name) => removeMutation.mutate(name)}
            />
          )}
        </Card.Content>
        <Card.Footer>
          <div className="flex w-full items-center justify-between gap-3">
            <Button
              variant="secondary"
              isPending={fetchMutation.isPending}
              isDisabled={remotes.length === 0 || isBusy}
              onPress={() => {
                setFetchResult(null);
                setActionError(null);
                fetchMutation.mutate();
              }}
            >
              立即更新
            </Button>
            <Button variant="primary" isDisabled={isBusy} onPress={() => setAddOpen(true)}>
              添加资源
            </Button>
          </div>
        </Card.Footer>
      </Card>

      {fetchResult && (
        <Alert status={fetchResult.warnings.length > 0 ? "warning" : "success"}>
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>更新完成</Alert.Title>
            <Alert.Description>
              成功拉取 {fetchResult.fetched} 个资源：脚本 {fetchResult.scripts}、重写 {fetchResult.rewrites}、任务{" "}
              {fetchResult.tasks}
              {fetchResult.warnings.length > 0 && `，警告 ${fetchResult.warnings.length} 条`}
            </Alert.Description>
            {fetchResult.warnings.length > 0 && (
              <ul className="mt-2 list-inside list-disc space-y-1 break-words text-sm">
                {fetchResult.warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
            )}
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

      <RemoteFormModal
        mode="add"
        open={addOpen}
        onClose={() => setAddOpen(false)}
        onSave={async (resource) => addMutation.mutateAsync(resource)}
        busy={addMutation.isPending}
        setError={setActionError}
      />

      <RemoteFormModal
        mode="edit"
        open={editOpen}
        initialData={editRemote}
        onClose={() => {
          setEditRemote(null);
          setEditOpen(false);
        }}
        onSave={async (resource) => updateMutation.mutateAsync(resource)}
        busy={updateMutation.isPending}
        setError={setActionError}
      />
    </div>
  );
}
