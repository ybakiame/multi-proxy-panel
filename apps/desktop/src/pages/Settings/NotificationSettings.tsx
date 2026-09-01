import { Button, Card, Chip, Switch } from "@heroui/react";
import { requestPermission } from "@tauri-apps/plugin-notification";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { toErrorMessage } from "../../api";
import { toastError } from "../../toast";

interface NotificationSettingsProps {
  settings: UseSettingsConfigReturn;
}

export default function NotificationSettings({ settings }: NotificationSettingsProps) {
  const { config, notifPerm, setNotifPerm, notifPermBusy, setNotifPermBusy, persist } = settings;

  return (
    <Card>
      <Card.Header>
        <Card.Title>Notification Settings</Card.Title>
        <Card.Description>Customize Android notification content</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        {/* Permission status */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <span className="text-sm">Notification Permission:</span>
            {notifPerm === "granted" && (
              <Chip size="sm" variant="soft" color="success">
                Granted
              </Chip>
            )}
            {notifPerm === "denied" && (
              <Chip size="sm" variant="soft" color="warning">
                Denied
              </Chip>
            )}
            {notifPerm === "unknown" && (
              <Chip size="sm" variant="soft" color="default">
                Unknown
              </Chip>
            )}
          </div>
          {notifPerm !== "granted" && (
            <div className="flex items-center gap-3">
              <Button
                variant="secondary"
                size="sm"
                isPending={notifPermBusy}
                onPress={() => {
                  setNotifPermBusy(true);
                  requestPermission()
                    .then((resp) => {
                      setNotifPerm(resp ? "granted" : "denied");
                    })
                    .catch((err) => {
                      toastError(toErrorMessage(err));
                    })
                    .finally(() => setNotifPermBusy(false));
                }}
              >
                Request Permission
              </Button>
              <span className="text-xs text-muted">Required for Android 13+</span>
            </div>
          )}
        </div>

        <Switch
          isSelected={config?.vpn_notify_show_traffic ?? true}
          onChange={(next) => {
            void persist({ vpn_notify_show_traffic: next });
          }}
        >
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
            Show upload/download traffic
          </Switch.Content>
        </Switch>
        <Switch
          isSelected={config?.vpn_notify_show_selection ?? true}
          onChange={(next) => {
            void persist({ vpn_notify_show_selection: next });
          }}
        >
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
            Show current proxy group and node
          </Switch.Content>
        </Switch>
      </Card.Content>
    </Card>
  );
}
