import { getApiKey, clearApiKey } from "../api/config";
import { validateApiKey } from "../api/client";
import { createContext, useContext, useEffect, useState, ReactNode } from "react";

interface AuthContextType {
  apiKey: string | null;
  isAuthenticated: boolean;
  login: (key: string) => Promise<void>;
  logout: () => void;
  isLoading: boolean;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [apiKey, setApiKey] = useState<string | null>(getApiKey);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    setApiKey(getApiKey());
  }, []);

  const login = async (key: string) => {
    setIsLoading(true);
    try {
      const trimmed = key.trim();
      await validateApiKey(trimmed);
      localStorage.setItem("pp_api_key", trimmed);
      setApiKey(trimmed);
    } finally {
      setIsLoading(false);
    }
  };

  const logout = () => {
    clearApiKey();
    setApiKey(null);
  };

  return (
    <AuthContext.Provider
      value={{
        apiKey,
        isAuthenticated: !!apiKey,
        login,
        logout,
        isLoading,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthContextType {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within AuthProvider");
  }
  return ctx;
}
