import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Button, Card } from "@heroui/react";
import { KeyIcon } from "@heroicons/react/24/outline";
import { useAuth } from "../context/AuthContext";
import { parseError } from "../api/client";
import { AxiosError } from "axios";
import { FormInput } from "../components/ui";

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

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-4">
      <Card className="w-full max-w-md">
        <Card.Header className="flex flex-col items-center gap-2 pb-2">
          <div className="rounded-full bg-primary/10 p-3">
            <KeyIcon className="h-8 w-8 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">{t("login.title")}</h1>
          <p className="text-sm text-muted-foreground">{t("login.subtitle")}</p>
        </Card.Header>
        <Card.Content>
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
            <Button type="submit" className="w-full" isPending={isLoading}>
              {t("login.verify")}
            </Button>
          </form>
        </Card.Content>
      </Card>
    </div>
  );
}
