import { Alert } from "@heroui/react";
import type { OpResult } from "./useSubscriptionData";

interface SubscriptionAlertsProps {
  anyEnabled: boolean;
  result: OpResult | null;
  error: string | null;
}

export function SubscriptionAlerts({ anyEnabled, result, error }: SubscriptionAlertsProps) {
  return (
    <>
      {anyEnabled && (
        <Alert status="accent">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>生效提示</Alert.Title>
            <Alert.Description>启用的订阅可在首页选择，首页选择的订阅唯一生效</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {result && (
        <Alert status={result.sub.error ? "warning" : "success"}>
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>
              {result.kind === "add"
                ? result.sub.error
                  ? "订阅已添加，但首次拉取失败"
                  : "订阅已添加"
                : result.sub.error
                  ? "刷新失败，已保留上次数据"
                  : "订阅已更新"}
            </Alert.Title>
            <Alert.Description className="break-all">
              {result.sub.error
                ? `「${result.sub.name}」${result.sub.error}`
                : `「${result.sub.name}」共 ${result.sub.node_count} 个节点`}
            </Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description className="break-all">{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}
    </>
  );
}
