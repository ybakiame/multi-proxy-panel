import { useState } from "react";
import { Button, Input, Label, ListBox, Select } from "@heroui/react";
import type { ProfileView } from "../../api";
import { coreLabel } from "./utils";
import { UA_PRESETS } from "./utils";

interface AddSubscriptionModalProps {
  isOpen: boolean;
  onClose: () => void;
  busy: boolean;
  coreProfiles: ProfileView[];
  clientCoreType: string | undefined;
  onAdd: (name: string, url: string, ua: string, profileId: string | null) => void;
}

export function AddSubscriptionModal({
  isOpen,
  onClose,
  busy,
  coreProfiles,
  clientCoreType,
  onAdd,
}: AddSubscriptionModalProps) {
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [ua, setUa] = useState("");
  const [profileId, setProfileId] = useState("");

  const formValid = name.trim().length > 0 && url.trim().length > 0;

  const handleClose = () => {
    setName("");
    setUrl("");
    setUa("");
    setProfileId("");
    onClose();
  };

  const handleAdd = () => {
    onAdd(name.trim(), url.trim(), ua.trim(), profileId === "" ? null : profileId);
    handleClose();
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-[480px] rounded-lg bg-background p-6 shadow-lg">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold">添加订阅</h2>
          <Button size="sm" variant="tertiary" onPress={handleClose}>
            ✕
          </Button>
        </div>
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-1">
            <Label htmlFor="sub-name">名称</Label>
            <Input
              id="sub-name"
              aria-label="订阅名称"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="我的机场"
              fullWidth
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="sub-url">URL</Label>
            <Input
              id="sub-url"
              aria-label="订阅 URL"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://example.com/sub"
              fullWidth
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="sub-ua">User-Agent（可选）</Label>
            <Input
              id="sub-ua"
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
            <Label htmlFor="sub-profile">关联覆写</Label>
            {coreProfiles.length > 0 ? (
              <Select
                id="sub-profile"
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
                    {coreProfiles.map((profile) => (
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
                当前核心（{coreLabel(clientCoreType)}）暂无覆写模板，可到「覆写」页创建
              </p>
            )}
          </div>
        </div>
        <div className="mt-6 flex justify-end gap-2">
          <Button variant="secondary" onPress={handleClose}>
            取消
          </Button>
          <Button variant="primary" isPending={busy} isDisabled={!formValid} onPress={handleAdd}>
            添加
          </Button>
        </div>
      </div>
    </div>
  );
}
