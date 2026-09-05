import { useState } from "react";
import { Alert, Button, Card, Chip } from "@heroui/react";
import clsx from "clsx";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createProfile, deleteProfile, getProfile, listProfiles, toErrorMessage, updateProfile } from "../api";
import { PROFILES_KEY, profileKey } from "../api/keys";
import type { CoreType, ProfileView } from "../api";
import { MobileBackHeader } from "../layout/mobile/MobileBackHeader";
import { CORE_CHIP_COLORS, CORE_LABELS, ProfileEditor } from "./OverrideEditor";
import { CreateProfileModal, DeleteProfileDialog } from "./OverrideModals";

export default function Override() {
  const queryClient = useQueryClient();

  const { data: profiles, isLoading: profilesLoading } = useQuery({
    queryKey: PROFILES_KEY,
    queryFn: listProfiles,
  });

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // 新建模板对话框
  const [createOpen, setCreateOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newCoreType, setNewCoreType] = useState<CoreType>("singbox");

  // 删除确认
  const [deleteTarget, setDeleteTarget] = useState<ProfileView | null>(null);

  const selectedProfile = profiles?.find((profile) => profile.id === selectedId) ?? null;

  // 使用 useQuery 获取模板详情，替代 selectProfile 回调里的手动 fetch + setState
  const { data: profileDetail, error: profileDetailError } = useQuery({
    queryKey: profileKey(selectedId),
    queryFn: async () => {
      if (!selectedId) return null;
      return getProfile(selectedId);
    },
    enabled: selectedId !== null,
    staleTime: 0,
  });

  const detail = profileDetail ?? null;

  const handleSelectProfile = (id: string) => {
    setSelectedId(id);
    setError(null);
    setSuccess(null);
  };

  const saveMutation = useMutation({
    mutationFn: updateProfile,
    onSuccess: () => {
      setSuccess(`模板「${detail?.name}」已保存，需重启代理后生效`);
      void queryClient.invalidateQueries({ queryKey: PROFILES_KEY });
      void queryClient.invalidateQueries({ queryKey: profileKey(selectedId) });
    },
    onError: (err) => setError(toErrorMessage(err)),
    onSettled: () => setBusy(false),
  });

  const handleSave = (input: {
    id: string;
    name: string;
    yaml_override: string;
    js_override: string;
    yaml_url: string;
    js_url: string;
  }) => {
    setBusy(true);
    setError(null);
    setSuccess(null);
    saveMutation.mutate(input);
  };

  const createMutation = useMutation({
    mutationFn: createProfile,
    onSuccess: (created) => {
      setSuccess(`模板「${created.name}」已创建`);
      setCreateOpen(false);
      setNewName("");
      setNewCoreType("singbox");
      void queryClient.invalidateQueries({ queryKey: PROFILES_KEY });
      void handleSelectProfile(created.id);
    },
    onError: (err) => setError(toErrorMessage(err)),
    onSettled: () => setBusy(false),
  });

  const handleCreate = async () => {
    setBusy(true);
    setError(null);
    setSuccess(null);
    createMutation.mutate({ name: newName.trim(), core_type: newCoreType });
  };

  const deleteMutation = useMutation({
    mutationFn: deleteProfile,
    onSuccess: () => {
      setSuccess(`模板「${deleteTarget?.name}」已删除`);
      if (selectedId === deleteTarget?.id) {
        setSelectedId(null);
      }
      setDeleteTarget(null);
      void queryClient.invalidateQueries({ queryKey: PROFILES_KEY });
    },
    onError: (err) => setError(toErrorMessage(err)),
    onSettled: () => setBusy(false),
  });

  const handleDelete = async () => {
    if (!deleteTarget) return;
    setBusy(true);
    setError(null);
    setSuccess(null);
    deleteMutation.mutate(deleteTarget.id);
  };

  // 将查询错误映射到 UI 错误（直接在渲染期处理，避免 useEffect + setState）
  const displayError = error ?? (profileDetailError ? toErrorMessage(profileDetailError) : null);

  return (
    <div className="flex flex-col gap-6">
      <MobileBackHeader title="覆写" />
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
            {(() => {
              const profileList = profiles;
              if (profilesLoading && (!profileList || profileList.length === 0)) {
                return (
                  <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                    <span className="text-sm text-muted">正在加载模板…</span>
                  </div>
                );
              }
              if (!profileList || profileList.length === 0) {
                return (
                  <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                    <span className="text-sm text-muted">暂无模板</span>
                    <span className="text-xs text-muted/80">点击下方「新建模板」创建首个覆写模板</span>
                  </div>
                );
              }
              return profileList.map((profile) => {
                const isSelected = profile.id === selectedId;
                return (
                  <div
                    key={profile.id}
                    className={clsx(
                      "flex flex-wrap items-center gap-3 rounded-lg border p-3 transition-colors",
                      isSelected ? "border-accent/60 bg-accent/5" : "border-border/70 bg-surface-secondary/40",
                    )}
                  >
                    <button
                      type="button"
                      onClick={() => void handleSelectProfile(profile.id)}
                      className="flex min-w-0 flex-1 flex-col items-start gap-1 text-left"
                    >
                      <span
                        className={clsx(
                          "max-w-full truncate text-sm",
                          isSelected ? "font-medium text-foreground" : "text-foreground/90",
                        )}
                        title={profile.name}
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
                        onPress={() => void handleSelectProfile(profile.id)}
                      >
                        编辑
                      </Button>
                      <Button size="sm" variant="tertiary" isDisabled={busy} onPress={() => setDeleteTarget(profile)}>
                        删除
                      </Button>
                    </div>
                  </div>
                );
              });
            })()}
          </Card.Content>
          <Card.Footer>
            <Button variant="primary" fullWidth isDisabled={busy} onPress={() => setCreateOpen(true)}>
              新建模板
            </Button>
          </Card.Footer>
        </Card>

        {/* 右侧：编辑器区 */}
        {selectedProfile && detail ? (
          <ProfileEditor
            key={selectedProfile.id}
            profile={selectedProfile}
            detail={detail}
            onSave={handleSave}
            busy={busy}
          />
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

      {displayError && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description>{displayError}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {/* 新建模板 */}
      <CreateProfileModal
        open={createOpen}
        onOpenChange={setCreateOpen}
        name={newName}
        onNameChange={setNewName}
        coreType={newCoreType}
        onCoreTypeChange={setNewCoreType}
        busy={busy}
        onSubmit={() => void handleCreate()}
      />

      {/* 删除确认 */}
      <DeleteProfileDialog
        target={deleteTarget}
        busy={busy}
        onCancel={() => setDeleteTarget(null)}
        onConfirm={() => void handleDelete()}
      />
    </div>
  );
}
