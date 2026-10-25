import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { KeyIcon, ShieldCheckIcon, ServerStackIcon, SignalIcon } from "@heroicons/react/24/outline";
import { useAuth } from "../context/AuthContext";
import { parseError } from "../api/client";
import { AxiosError } from "axios";
import { BrandMark } from "../components/BrandMark";
import { VersionBadge } from "../components/VersionBadge";
import { FormInput } from "../components/ui";
import { Button } from "../components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../components/ui/card";
import { Spinner } from "../components/ui/spinner";

export function Login() {
  const { t } = useTranslation();
  const { login, isLoading } = useAuth();
  const navigate = useNavigate();
  const [key, setKey] = useState("");
  const [error, setError] = useState("");

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    try {
      await login(key.trim());
      navigate("/", { replace: true });
    } catch (err) {
      const apiErr = parseError(err as AxiosError);
      if (apiErr.status === 401) {
        setError(t("login.invalid"));
      } else {
        setError(t("login.failed", { error: apiErr.message }));
      }
    }
  };

  const highlights = [
    { icon: ServerStackIcon, labelKey: "login.highlightNodes" },
    { icon: ShieldCheckIcon, labelKey: "login.highlightSecurity" },
    { icon: SignalIcon, labelKey: "login.highlightObserve" },
  ];

  return (
    <div className="grid min-h-screen bg-background lg:grid-cols-[1.05fr_1fr]">
      {/* 品牌区：湖水青渐变，仅此处与重点 KPI 允许渐变 */}
      <div className="relative hidden flex-col justify-between bg-(--gradient-brand) p-10 text-primary-foreground lg:flex xl:p-14">
        <div className="flex items-center gap-3">
          <BrandMark className="h-10 w-10" />
          <span className="text-lg font-semibold tracking-tight">ProxyPanel</span>
        </div>
        <div className="max-w-md space-y-6">
          <h1 className="text-3xl leading-tight font-bold tracking-tight xl:text-4xl">
            {t("login.brandTitle")}
          </h1>
          <p className="text-sm leading-relaxed text-primary-foreground/85">
            {t("login.brandSubtitle")}
          </p>
          <ul className="space-y-3 pt-2">
            {highlights.map(({ icon: Icon, labelKey }) => (
              <li key={labelKey} className="flex items-center gap-2.5 text-sm">
                <span className="flex size-7 items-center justify-center rounded-lg bg-primary-foreground/15">
                  <Icon className="h-4 w-4" />
                </span>
                {t(labelKey)}
              </li>
            ))}
          </ul>
        </div>
        <VersionBadge className="text-primary-foreground/80" withStatus />
      </div>

      {/* 表单区 */}
      <div className="flex items-center justify-center p-4 md:p-8">
        <div className="w-full max-w-sm">
          <div className="mb-6 flex flex-col items-center gap-3 lg:hidden">
            <BrandMark className="h-11 w-11" />
            <span className="text-lg font-semibold tracking-tight">ProxyPanel</span>
          </div>
          <Card>
            <CardHeader className="items-center gap-2 pb-2 text-center">
              <div className="rounded-xl bg-accent p-3">
                <KeyIcon className="h-6 w-6 text-accent-foreground" />
              </div>
              <CardTitle className="text-xl font-bold">
                <h1>{t("login.title")}</h1>
              </CardTitle>
              <CardDescription>{t("login.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={handleSubmit} className="space-y-4">
                <FormInput
                  type="password"
                  label={t("login.apiKey")}
                  placeholder={t("login.apiKeyPlaceholder")}
                  value={key}
                  onChange={setKey}
                  isInvalid={!!error}
                  errorMessage={error}
                  isRequired
                />
                <Button type="submit" className="w-full" disabled={isLoading}>
                  {isLoading && <Spinner aria-label={t("common.loading")} />}
                  {t("login.verify")}
                </Button>
              </form>
            </CardContent>
          </Card>
          <div className="mt-4 flex justify-center lg:hidden">
            <VersionBadge withStatus />
          </div>
        </div>
      </div>
    </div>
  );
}
