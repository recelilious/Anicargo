import { useEffect, useState } from "react";
import "../Styles/theme.css";
import "../Styles/PageLogin.css";
import LogoMark from "../Icons/LogoMark";
import FooterNote from "./Components/FooterNote";
import ErrorBanner from "./Components/ErrorBanner";
import { apiFetch } from "../api";
import { useNavigate } from "react-router-dom";

interface SignupResponse {
  user_id: string;
}

export default function PageSignup() {
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [invite, setInvite] = useState("");
  const [agree, setAgree] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();

  const disabled = loading;

  useEffect(() => {
    if (!error) return;
    const t = window.setTimeout(() => setError(null), 4000);
    return () => window.clearTimeout(t);
  }, [error]);

  const eyeIcon = (
    <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path
        d="M2 12c1.5-3 5.2-6 10-6s8.5 3 10 6c-1.5 3-5.2 6-10 6s-8.5-3-10-6z"
        stroke="currentColor"
        strokeWidth="1.6"
        fill="none"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="12" cy="12" r="3.2" stroke="currentColor" strokeWidth="1.6" fill="none" />
    </svg>
  );

  const eyeOffIcon = (
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
  );

  function validate(): string | null {
    if (!username.trim()) {
      return "Username is required.";
    }
    if (!password) {
      return "Password is required.";
    }
    if (!confirm) {
      return "Please confirm your password.";
    }
    if (!invite.trim()) {
      return "Invite code is required.";
    }
    if (password.length < 6) {
      return "Password must be at least 6 characters.";
    }
    if (!/^[\x00-\x7F]+$/.test(password)) {
      return "Password must use ASCII characters only.";
    }
    if (password !== confirm) {
      return "Passwords do not match.";
    }
    if (!agree) {
      return "Please agree to the terms to continue.";
    }
    return null;
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);

    const validation = validate();
    if (validation) {
      setError(validation);
      return;
    }

    setLoading(true);
    try {
      await apiFetch<SignupResponse>("/api/users", {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          user_id: username.trim(),
          password,
          invite_code: invite.trim()
        })
      });
      navigate("/login", { replace: true });
    } catch (err) {
      const message = (err as Error).message ?? "";
      const statusMatch = message.match(/Request failed \((\d+)/);
      const status = statusMatch ? Number(statusMatch[1]) : undefined;
      const lower = message.toLowerCase();
      const isNetwork = lower.includes("failed to fetch") || lower.includes("network");
      const isServer = typeof status === "number" && status >= 500;

      if (isNetwork || isServer) {
        setError("Server error. Please contact administrator.");
      } else if (status === 409 || lower.includes("exists")) {
        setError("User already exists.");
      } else {
        setError("Invite code is invalid.");
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
      <div className="login-card" role="main" aria-labelledby="signup-heading">
        <div className="login-brand" aria-hidden="true">
          <span className="brand-name">Anicargo</span>
          <LogoMark size={52} />
        </div>
        <h1 id="signup-heading" className="login-title">Sign up</h1>
        <form className="login-form" onSubmit={handleSubmit}>
          <label>
            <span>Username</span>
            <input
              type="text"
              name="username"
              autoComplete="username"
              placeholder="choose username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              disabled={disabled}
            />
          </label>
          <label>
            <span>Password</span>
            <div className="password-field">
              <input
                type={showPassword ? "text" : "password"}
                name="password"
                autoComplete="new-password"
                placeholder="enter password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                disabled={disabled}
              />
              <button
                type="button"
                className="password-toggle"
                aria-label={showPassword ? "Hide password" : "Show password"}
                onClick={() => setShowPassword((p) => !p)}
              >
                {showPassword ? eyeIcon : eyeOffIcon}
              </button>
            </div>
          </label>
          <label>
            <span>Confirm password</span>
            <div className="password-field">
              <input
                type={showConfirm ? "text" : "password"}
                name="confirm"
                autoComplete="new-password"
                placeholder="confirm password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                disabled={disabled}
              />
              <button
                type="button"
                className="password-toggle"
                aria-label={showConfirm ? "Hide password" : "Show password"}
                onClick={() => setShowConfirm((p) => !p)}
              >
                {showConfirm ? eyeIcon : eyeOffIcon}
              </button>
            </div>
          </label>
          <label>
            <span>Invite code</span>
            <input
              type="text"
              name="invite"
              autoComplete="one-time-code"
              placeholder="enter invite code"
              value={invite}
              onChange={(e) => setInvite(e.target.value)}
              disabled={disabled}
            />
          </label>
          <div className="login-row">
            <label className="remember">
              <input
                type="checkbox"
                name="agree"
                checked={agree}
                onChange={(e) => setAgree(e.target.checked)}
                disabled={disabled}
              />
              <span>I agree to the terms</span>
            </label>
            <span className="text-link" aria-hidden="true">&nbsp;</span>
          </div>
          <button type="submit" className="login-btn" disabled={disabled}>
            {loading ? "signing up..." : "sign up"}
          </button>
        </form>
        <div className="login-meta">
          <span>Already have an account?</span>
          <a className="text-link" href="/login">Log in</a>
        </div>
      </div>
      <FooterNote className="login-footer-note" />
    </div>
  );
}
