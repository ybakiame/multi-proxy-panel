import { useEffect } from "react";
import { Routes, Route, useNavigate } from "react-router-dom";
import { useAuth } from "./context/AuthContext";
import { Layout } from "./components/Layout";
import { Login } from "./pages/Login";
import { Dashboard } from "./pages/Dashboard";
import { Nodes } from "./pages/Nodes";
import { Protocols } from "./pages/Protocols";
import { CoreVersions } from "./pages/CoreVersions";
import { Certificates } from "./pages/Certificates";
import { Bindings } from "./pages/Bindings";
import { Hosts } from "./pages/Hosts";
import { Clients } from "./pages/Clients";
import { Groups } from "./pages/Groups";
import { Subscriptions } from "./pages/Subscriptions";
import { Metrics } from "./pages/Metrics";
import { Logs } from "./pages/Logs";
import { ApiKeys } from "./pages/ApiKeys";
import { Webhooks } from "./pages/Webhooks";
import { Onlines } from "./pages/Onlines";
import { Traffic } from "./pages/Traffic";
import { RelayRules } from "./pages/RelayRules";

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuth();
  const navigate = useNavigate();

  useEffect(() => {
    if (!isAuthenticated) {
      navigate("/login", { replace: true });
    }
  }, [isAuthenticated, navigate]);

  return isAuthenticated ? <Layout>{children}</Layout> : null;
}

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route
        path="/"
        element={
          <ProtectedRoute>
            <Dashboard />
          </ProtectedRoute>
        }
      />
      <Route
        path="/nodes"
        element={
          <ProtectedRoute>
            <Nodes />
          </ProtectedRoute>
        }
      />
      <Route
        path="/protocols"
        element={
          <ProtectedRoute>
            <Protocols />
          </ProtectedRoute>
        }
      />
      <Route
        path="/core-versions"
        element={
          <ProtectedRoute>
            <CoreVersions />
          </ProtectedRoute>
        }
      />
      <Route
        path="/certificates"
        element={
          <ProtectedRoute>
            <Certificates />
          </ProtectedRoute>
        }
      />
      <Route
        path="/bindings"
        element={
          <ProtectedRoute>
            <Bindings />
          </ProtectedRoute>
        }
      />
      <Route
        path="/hosts"
        element={
          <ProtectedRoute>
            <Hosts />
          </ProtectedRoute>
        }
      />
      <Route
        path="/clients"
        element={
          <ProtectedRoute>
            <Clients />
          </ProtectedRoute>
        }
      />
      <Route
        path="/groups"
        element={
          <ProtectedRoute>
            <Groups />
          </ProtectedRoute>
        }
      />
      <Route
        path="/subscriptions"
        element={
          <ProtectedRoute>
            <Subscriptions />
          </ProtectedRoute>
        }
      />
      <Route
        path="/metrics"
        element={
          <ProtectedRoute>
            <Metrics />
          </ProtectedRoute>
        }
      />
      <Route
        path="/logs"
        element={
          <ProtectedRoute>
            <Logs />
          </ProtectedRoute>
        }
      />
      <Route
        path="/api-keys"
        element={
          <ProtectedRoute>
            <ApiKeys />
          </ProtectedRoute>
        }
      />
      <Route
        path="/webhooks"
        element={
          <ProtectedRoute>
            <Webhooks />
          </ProtectedRoute>
        }
      />
      <Route
        path="/onlines"
        element={
          <ProtectedRoute>
            <Onlines />
          </ProtectedRoute>
        }
      />
      <Route
        path="/traffic"
        element={
          <ProtectedRoute>
            <Traffic />
          </ProtectedRoute>
        }
      />
      <Route
        path="/relay-rules"
        element={
          <ProtectedRoute>
            <RelayRules />
          </ProtectedRoute>
        }
      />
    </Routes>
  );
}
