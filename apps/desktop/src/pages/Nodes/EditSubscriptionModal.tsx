import { useEffect, useState } from "react";
import { Button, Input, Label, ListBox, Select } from "@heroui/react";
import type { ProfileView, SubscriptionView } from "../../api";
import { coreLabel, subCoreType, UA_PRESETS } from "./utils";

interface EditSubscriptionModalProps {
  sub: SubscriptionView | null;
  busy: boolean;
  profiles: ProfileView[];
  clientCoreType: string | undefined;
  onClose: () => void;
  onSave: (sub: SubscriptionView, name: string, url: string, profileId: string | null, userAgent?: string) => void;
}

export function EditSubscriptionModal({
  sub,
  busy,
  profiles,
  clientCoreType,
  onClose,
  onSave,
}: EditSubscriptionModalProps) {
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [ua, setUa] = useState("");
  const [profileId, setProfileId] = useState("");

  useEffect(() => {
    if (sub) {
      setName(sub.name);
      setUrl(sub.url);
      setUa(sub.user_agent ?? "");
      setProfileId(sub.profile_id ?? "");
    }
  }, [sub?.id]);

  if (!sub) return null;

  const editCoreType = subCoreType(sub.format) ?? clientCoreType;
  const editCoreProfiles = profiles.filter((p) => p.core_type === editCoreType);
  const formValid = name.trim().length > 0 && url.trim().length > 0;

  const handleClose = () => {
    onClose();
  };

  const handleSave = () => {
    onSave(sub, name.trim(), url.trim(), profileId === "" ? null : profileId, ua.trim() || undefined);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-[480px] rounded-lg bg-background p-6 shadow-lg">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold">编辑订阅</h2>
          <Button size="sm" variant="tertiary" onPress={handleClose}>
            ✕
          </Button>
        </div>
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-1">
            <Label htmlFor="sub-edit-name">名称</Label>
            <Input
              id="sub-edit-name"
              aria-label="订阅名称"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="我的机场"
              fullWidth
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="sub-edit-url">URL</Label>
            <Input
              id="sub-edit-url"
              aria-label="订阅 URL"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://example.com/sub"
              fullWidth
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="sub-edit-ua">User-Agent（可选）</Label>
            <Input
              id="sub-edit-ua"
              aria-label="订阅 User-Agent"
              value={ua}
              onChange={(event) => setUa(event.target.value)}
              placeholder="留空使用默认 clash.meta"
              fullWidth
            />
            <div className="flex flex-wrap items-center gap-1.5 pt-1">
              <span className="text-xs text-muted">常用：</span>
              {UA_PRESETS.map((preset) => (
                <Button
                  key={preset.value}
                  size="sm"
                  variant={ua === preset.value ? "primary" : "secondary"}
                  onPress={() => setUa(preset.value)}
                >
                  {preset.label}
                </Button>
              ))}
            </div>
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="sub-edit-profile">关联覆写</Label>
            {editCoreProfiles.length > 0 ? (
              <Select
                id="sub-edit-profile"
                aria-label="关联覆写"
                placeholder="选择覆写模板"
                value={profileId}
                onChange={(value) => setProfileId(String(value ?? ""))}
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    <ListBox.Item id="" textValue="不关联">
                      不关联
                      <ListBox.ItemIndicator />
                    </ListBox.Item>
                    {editCoreProfiles.map((profile) => (
                      <ListBox.Item key={profile.id} id={profile.id} textValue={profile.name}>
                        {profile.name}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            ) : (
              <p className="text-xs text-warning">
                当前核心（{coreLabel(editCoreType)}）暂无覆写模板，可到「覆写」页创建
              </p>
            )}
          </div>
        </div>
        <div className="mt-6 flex justify-end gap-2">
          <Button variant="secondary" onPress={handleClose}>
            取消
          </Button>
          <Button variant="primary" isPending={busy} isDisabled={!formValid} onPress={handleSave}>
            保存
          </Button>
        </div>
      </div>
    </div>
  );
}
