import { useQuery } from "@tanstack/react-query";
import { Button } from "@pp/ui";
import { getAppVersion, openProjectUrl, toErrorMessage, toastError } from "@pp/client-core";

/** 双端共用应用版本和项目入口；版本来自壳，链接由系统浏览器处理。 */
export function AboutInfo() {
  const { data: version, isLoading } = useQuery({
    queryKey: ["app_version"],
    queryFn: getAppVersion,
    staleTime: Infinity,
  });
  const openProject = async () => {
    try {
      await openProjectUrl();
    } catch (err) {
      toastError(`打开项目链接失败：${toErrorMessage(err)}`);
    }
  };
  return (
    <>
      <div className="flex min-h-8 items-center justify-between gap-4">
        <span className="shrink-0 text-sm text-muted">版本号</span>
        <span className="text-right text-sm font-medium">{version ?? (isLoading ? "读取中…" : "未知")}</span>
      </div>
      <div className="flex min-h-8 items-center justify-between gap-4">
        <span className="shrink-0 text-sm text-muted">项目链接</span>
        <Button variant="secondary" size="sm" onPress={() => void openProject()}>
          GitHub
        </Button>
      </div>
    </>
  );
}
