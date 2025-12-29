import { useEffect, useMemo, useState } from "react";
import "../Styles/theme.css";
import "../Styles/PageLogin.css";
import LogoMark from "../Icons/LogoMark";
import FooterNote from "./Components/FooterNote";
import ErrorBanner from "./Components/ErrorBanner";
import { apiFetch } from "../api";
import { useSession, type SessionInfo } from "../session";
import { useNavigate } from "react-router-dom";

interface LoginResponse {
  token: string;
  user_id: string;
  role: "admin" | "user";
  expires_in: number;
}

export default function PageLogin() {
  const [showPassword, setShowPassword] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [lockUntil, setLockUntil] = useState<number | null>(null);
  const [attempts, setAttempts] = useState<number[]>([]);
  const { setSession } = useSession();
  const navigate = useNavigate();

  const locked = useMemo(() => (lockUntil ?? 0) > Date.now(), [lockUntil]);

  useEffect(() => {
    if (!locked && lockUntil) {
      setLockUntil(null);
    }
    if (locked && lockUntil) {
      const timer = window.setTimeout(() => setLockUntil(null), lockUntil - Date.now());
      return () => window.clearTimeout(timer);
    }
    return undefined;
  }, [locked, lockUntil]);

  // auto clear error after 4s
  useEffect(() => {
    if (!error) return;
    const t = window.setTimeout(() => setError(null), 4000);
    return () => window.clearTimeout(t);
  }, [error]);

  function validateInputs(): string | null {
    if (!username.trim()) {
      return "Username is required.";
    }
    if (!password) {
      return "Password is required.";
    }
    if (password.length < 6) {
      return "Password must be at least 6 characters.";
    }
    if (!/^[\x00-\x7F]+$/.test(password)) {
      return "Password must use ASCII characters only.";
    }
    return null;
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);

    const now = Date.now();
    if (locked) {
      setError("Too many attempts. Please wait a moment.");
      return;
    }

    const attemptWindow = attempts.filter((t) => now - t <= 10_000);
    if (attemptWindow.length >= 5) {
      const lockUntilTs = now + 5000;
      setLockUntil(lockUntilTs);
      setError("Too many attempts. Please wait 5 seconds.");
      return;
    }

    const validationError = validateInputs();
    if (validationError) {
      setAttempts([...attemptWindow, now]);
      setError(validationError);
      return;
    }

    setLoading(true);
    setAttempts([...attemptWindow, now]);
    try {
      const response = await apiFetch<LoginResponse>("/api/auth/login", {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          user_id: username.trim(),
          password
        })
      });

      const nextSession: SessionInfo = {
        token: response.token,
        userId: response.user_id,
        role: response.role,
        expiresIn: response.expires_in
      };
      setSession(nextSession);
      navigate("/library", { replace: true });
    } catch (err) {
      const message = (err as Error).message ?? "";
      const statusMatch = message.match(/Request failed \((\d+)/);
      const status = statusMatch ? Number(statusMatch[1]) : undefined;
      const lower = message.toLowerCase();
      const isNetwork = lower.includes("failed to fetch") || lower.includes("network");
      const isServer = typeof status === "number" && status >= 500;
      if (isNetwork || isServer) {
        setError("Server error. Please contact administrator.");
      } else {
        setError("Username does not exist or password is incorrect.");
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="login-shell">
      {error ? (
        <div className="error-banner-overlay">
          <button type="button" className="error-dismiss" onClick={() => setError(null)}>
            <ErrorBanner message={error} />
          </button>
        </div>
      ) : null}
      <div className="login-card" role="main" aria-labelledby="login-heading">
        <div className="login-brand" aria-hidden="true">
          <span className="brand-name">Anicargo</span>
          <LogoMark size={52} />
        </div>
        <h1 id="login-heading" className="login-title">Log in</h1>
        <form className="login-form" onSubmit={handleSubmit}>
          <label>
            <span>Username</span>
            <input
              type="text"
              name="username"
              autoComplete="username"
              placeholder="enter username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              disabled={loading || locked}
            />
          </label>
          <label>
            <span>Password</span>
            <div className="password-field">
              <input
                type={showPassword ? "text" : "password"}
                name="password"
                autoComplete="current-password"
                placeholder="enter password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                disabled={loading || locked}
              />
              <button
                type="button"
                className="password-toggle"
                aria-label={showPassword ? "Hide password" : "Show password"}
                onClick={() => setShowPassword((prev) => !prev)}
              >
                {!showPassword ? (
                  <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                    <path
                      d="M3 3l18 18M9.9 9.9a4 4 0 014.2 4.2M12 6c4.8 0 8.5 3.2 10 6-1.1 2.3-3.3 4.6-6 5.5M6.5 6.5C4.3 7.8 2.6 9.8 2 12c1 2.4 3.2 4.8 6 5.6"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      fill="none"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                    <path
                      d="M9.9 9.9a4 4 0 015.2 5.2"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      fill="none"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                ) : (
                  <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                    <path
                      d="M2 12c1.5-3 5.2-6 10-6s8.5 3 10 6c-1.5 3-5.2 6-10 6s-8.5-3-10-6z"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      fill="none"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                    <circle
                      cx="12"
                      cy="12"
                      r="3.2"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      fill="none"
                    />
                  </svg>
                )}
              </button>
            </div>
          </label>
          <div className="login-row">
            <label className="remember">
              <input type="checkbox" name="remember" disabled={loading || locked} />
              <span>Stay signed in</span>
            </label>
            <a className="text-link" href="#forgot">Forgot password?</a>
          </div>
          <button type="submit" className="login-btn" disabled={loading || locked}>
            {locked ? "please wait" : loading ? "logging in..." : "log in"}
          </button>
        </form>
        <div className="login-meta">
          <span>New here?</span>
          <a className="text-link" href="/signup">Create account</a>
        </div>
      </div>
      <FooterNote className="login-footer-note" />
    </div>
  );
}
