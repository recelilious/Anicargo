import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { apiFetch } from "../api";
import { SessionInfo, UserRole, useSession } from "../session";

interface LoginResponse {
  token: string;
  user_id: string;
  role: UserRole;
  expires_in: number;
}

export default function LoginPage() {
  const { session, setSession } = useSession();
  const [authTab, setAuthTab] = useState<"login" | "signup">("login");
  const [status, setStatus] = useState<string | null>(null);
  const [loginForm, setLoginForm] = useState({ userId: "", password: "" });
  const [signupForm, setSignupForm] = useState({
    userId: "",
    password: "",
    inviteCode: ""
  });
  const navigate = useNavigate();

  useEffect(() => {
    if (session) {
      navigate("/library", { replace: true });
    }
  }, [navigate, session]);

  async function handleLogin(event: React.FormEvent) {
    event.preventDefault();
    setStatus(null);
    try {
      const response = await apiFetch<LoginResponse>("/api/auth/login", {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          user_id: loginForm.userId,
          password: loginForm.password
        })
      });

      const nextSession: SessionInfo = {
        token: response.token,
        userId: response.user_id,
        role: response.role,
        expiresIn: response.expires_in
      };
      setSession(nextSession);
      setLoginForm({ userId: "", password: "" });
      setStatus("Signed in. Redirecting...");
      navigate("/library");
    } catch (error) {
      setStatus((error as Error).message);
    }
  }

  async function handleSignup(event: React.FormEvent) {
    event.preventDefault();
    setStatus(null);
    try {
      await apiFetch<{ user_id: string }>("/api/users", {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          user_id: signupForm.userId,
          password: signupForm.password,
          invite_code: signupForm.inviteCode
        })
      });
      setAuthTab("login");
      setSignupForm({ userId: "", password: "", inviteCode: "" });
      setStatus("User created. You can sign in now.");
    } catch (error) {
      setStatus((error as Error).message);
    }
  }

  return (
    <div className="grid">
      <section className="panel span-6">
        <div className="panel-header">
          <h2>Access</h2>
          <div className="tabs">
            <button
              className={authTab === "login" ? "tab active" : "tab"}
              onClick={() => setAuthTab("login")}
              type="button"
            >
              Login
            </button>
            <button
              className={authTab === "signup" ? "tab active" : "tab"}
              onClick={() => setAuthTab("signup")}
              type="button"
            >
              Create user
            </button>
          </div>
        </div>

        {authTab === "login" ? (
          <form className="form" onSubmit={handleLogin}>
            <label>
              User ID
              <input
                value={loginForm.userId}
                onChange={(event) =>
                  setLoginForm({ ...loginForm, userId: event.target.value })
                }
                placeholder="e.g. admin"
                required
              />
            </label>
            <label>
              Password
              <input
                type="password"
                value={loginForm.password}
                onChange={(event) =>
                  setLoginForm({ ...loginForm, password: event.target.value })
                }
                placeholder="••••••••"
                required
              />
            </label>
            <button className="primary" type="submit">
              Unlock library
            </button>
          </form>
        ) : (
          <form className="form" onSubmit={handleSignup}>
            <label>
              User ID
              <input
                value={signupForm.userId}
                onChange={(event) =>
                  setSignupForm({ ...signupForm, userId: event.target.value })
                }
                placeholder="choose a handle"
                required
              />
            </label>
            <label>
              Password
              <input
                type="password"
                value={signupForm.password}
                onChange={(event) =>
                  setSignupForm({ ...signupForm, password: event.target.value })
                }
                placeholder="at least 8 chars"
                required
              />
            </label>
            <label>
              Invite code
              <input
                value={signupForm.inviteCode}
                onChange={(event) =>
                  setSignupForm({ ...signupForm, inviteCode: event.target.value })
                }
                placeholder="invitecode"
                required
              />
            </label>
            <button className="primary" type="submit">
              Create account
            </button>
          </form>
        )}
        {status ? <p className="status-line">{status}</p> : null}
      </section>

      <section className="panel span-6">
        <h2>What you get</h2>
        <p className="subtitle">
          Anicargo streams from your server, keeps your library behind auth, and
          generates HLS playlists on demand. Log in to unlock the library, or
          create a new user with the invite code.
        </p>
        <div className="stack">
          <div className="stack-item">
            <h3>Stream-ready</h3>
            <p>HLS playlists are generated on the fly for each title.</p>
          </div>
          <div className="stack-item">
            <h3>Access control</h3>
            <p>JWT tokens protect every playlist and segment request.</p>
          </div>
          <div className="stack-item">
            <h3>Self-hosted</h3>
            <p>Keep everything on your own Linux server and scale later.</p>
          </div>
        </div>
      </section>
    </div>
  );
}
