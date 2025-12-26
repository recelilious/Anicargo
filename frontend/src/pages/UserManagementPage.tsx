import { useState } from "react";
import { apiFetch, apiFetchEmpty } from "../api";
import { useSession } from "../session";

export default function UserManagementPage() {
  const { session, clearSession } = useSession();
  const [createForm, setCreateForm] = useState({
    userId: "",
    password: "",
    inviteCode: ""
  });
  const [deleteId, setDeleteId] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  async function handleCreate(event: React.FormEvent) {
    event.preventDefault();
    setStatus(null);
    try {
      await apiFetch<{ user_id: string }>("/api/users", {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          user_id: createForm.userId,
          password: createForm.password,
          invite_code: createForm.inviteCode
        })
      }, session?.token);
      setCreateForm({ userId: "", password: "", inviteCode: "" });
      setStatus("User created.");
    } catch (error) {
      setStatus((error as Error).message);
    }
  }

  async function handleDeleteSelf() {
    if (!session) {
      return;
    }
    setStatus(null);
    try {
      await apiFetchEmpty(`/api/users/${session.userId}`, { method: "DELETE" }, session.token);
      clearSession();
      setStatus("Account deleted. You are signed out.");
    } catch (error) {
      setStatus((error as Error).message);
    }
  }

  async function handleDeleteUser(event: React.FormEvent) {
    event.preventDefault();
    if (!deleteId) {
      return;
    }
    setStatus(null);
    try {
      await apiFetchEmpty(`/api/users/${deleteId}`, { method: "DELETE" }, session?.token);
      setDeleteId("");
      setStatus("User deleted.");
    } catch (error) {
      setStatus((error as Error).message);
    }
  }

  return (
    <div className="grid">
      <section className="panel span-6">
        <div className="panel-header">
          <h2>Create user</h2>
          <span className="pill">Invite required</span>
        </div>
        <form className="form" onSubmit={handleCreate}>
          <label>
            User ID
            <input
              value={createForm.userId}
              onChange={(event) =>
                setCreateForm({ ...createForm, userId: event.target.value })
              }
              placeholder="new user id"
              required
            />
          </label>
          <label>
            Password
            <input
              type="password"
              value={createForm.password}
              onChange={(event) =>
                setCreateForm({ ...createForm, password: event.target.value })
              }
              placeholder="temporary password"
              required
            />
          </label>
          <label>
            Invite code
            <input
              value={createForm.inviteCode}
              onChange={(event) =>
                setCreateForm({ ...createForm, inviteCode: event.target.value })
              }
              placeholder="invitecode"
              required
            />
          </label>
          <button className="primary" type="submit">
            Create account
          </button>
        </form>
      </section>

      <section className="panel span-6">
        <div className="panel-header">
          <h2>Account controls</h2>
          <span className="pill">{session?.role ?? "unknown"}</span>
        </div>
        <div className="stack">
          <div className="stack-item">
            <h3>Delete your account</h3>
            <p>
              Removes your user and revokes access. This action cannot be
              undone.
            </p>
            <button className="danger" type="button" onClick={handleDeleteSelf}>
              Delete my account
            </button>
          </div>
          {session?.role === "admin" ? (
            <div className="stack-item">
              <h3>Delete another user</h3>
              <form className="form inline" onSubmit={handleDeleteUser}>
                <input
                  value={deleteId}
                  onChange={(event) => setDeleteId(event.target.value)}
                  placeholder="user id"
                  required
                />
                <button className="ghost" type="submit">
                  Delete
                </button>
              </form>
            </div>
          ) : null}
        </div>
        {status ? <p className="status-line">{status}</p> : null}
      </section>
    </div>
  );
}
