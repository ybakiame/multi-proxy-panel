/**
 * RemotesTab page component — composition of table, modals, and fetch result.
 */

import { useCallback, useState } from "react";
import { Alert, Button, Card } from "@heroui/react";
import {
  addRemote,
  fetchRemotes,
  getRemoteIcon,
  listRemotes,
  removeRemote,
  updateRemote,
  toErrorMessage,
} from "../../../api";
import type { FetchReport, RemoteResource } from "../../../api";
import RemoteFormModal from "./RemoteFormModal";
import RemoteTable from "./RemoteTable";

interface RemotesTabProps {
  remotes: RemoteResource[];
  setRemotes: React.Dispatch<React.SetStateAction<RemoteResource[]>>;
  iconCache: Record<string, string>;
  setIconCache: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  busy: boolean;
  setBusy: React.Dispatch<React.SetStateAction<boolean>>;
  error: string | null;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  fetchResult: FetchReport | null;
  setFetchResult: React.Dispatch<React.SetStateAction<FetchReport | null>>;
}

export default function RemotesTab({
  remotes,
  setRemotes,
  iconCache,
  setIconCache,
  busy,
  setBusy,
  setError,
  fetchResult,
  setFetchResult,
}: RemotesTabProps) {
  const [addOpen, setAddOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [editRemote, setEditRemote] = useState<RemoteResource | null>(null);

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
  }, [setRemotes, setIconCache, setError]);

  const handleAdd = async (resource: RemoteResource) => {
    setBusy(true);
    setError(null);
    try {
      await addRemote(resource);
      setAddOpen(false);
      await refreshRemotes();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async (name: string) => {
    setBusy(true);
    setError(null);
    try {
      await removeRemote(name);
      await refreshRemotes();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleToggle = async (remote: RemoteResource) => {
    const next = { ...remote, enabled: !remote.enabled };
    setBusy(true);
    setError(null);
    try {
      await removeRemote(remote.name);
      await addRemote(next);
      await refreshRemotes();
    } catch (err) {
      setError(toErrorMessage(err));
      await refreshRemotes();
    } finally {
      setBusy(false);
    }
  };

  const handleOpenEdit = (remote: RemoteResource) => {
    setEditRemote(remote);
    setError(null);
    setEditOpen(true);
  };

  const handleEditSave = async (resource: RemoteResource) => {
    if (!editRemote) return;
    setBusy(true);
    setError(null);
    try {
      await updateRemote(resource);
      setEditOpen(false);
      setEditRemote(null);
      await refreshRemotes();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleFetch = async () => {
    setBusy(true);
    setError(null);
    setFetchResult(null);
    try {
      setFetchResult(await fetchRemotes());
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
          <Card.Title>远程资源</Card.Title>
          <Card.Description>脚本 / 配置片段订阅，按间隔拉取并落盘缓存</Card.Description>
        </Card.Header>
        <Card.Content>
          <RemoteTable
            remotes={remotes}
            iconCache={iconCache}
            busy={busy}
            onToggle={handleToggle}
            onEdit={handleOpenEdit}
            onRemove={handleRemove}
          />
        </Card.Content>
        <Card.Footer>
          <div className="flex w-full items-center justify-between gap-3">
            <Button
              variant="secondary"
              isPending={busy}
              isDisabled={remotes.length === 0}
              onPress={() => void handleFetch()}
            >
              立即更新
            </Button>
            <Button variant="primary" isDisabled={busy} onPress={() => setAddOpen(true)}>
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

      <RemoteFormModal
        mode="add"
        open={addOpen}
        onClose={() => setAddOpen(false)}
        onSave={handleAdd}
        busy={busy}
        setError={setError}
      />

      <RemoteFormModal
        mode="edit"
        open={editOpen}
        initialData={editRemote}
        onClose={() => {
          setEditRemote(null);
          setEditOpen(false);
        }}
        onSave={handleEditSave}
        busy={busy}
        setError={setError}
      />
    </div>
  );
}
