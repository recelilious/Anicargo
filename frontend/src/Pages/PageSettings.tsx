import { useEffect, useMemo, useState } from "react";
import AppShell from "../Components/AppShell";
import { apiFetch } from "../api";
import { useSession } from "../session";
import { applyLocalTheme, loadLocalTheme, resetLocalTheme, saveLocalTheme } from "../theme";
import "../Styles/PageSettings.css";

interface UserSettingsResponse {
  display_name?: string | null;
  theme: string;
  playback_speed: number;
  subtitle_lang?: string | null;
}

export default function PageSettings() {
  const { session } = useSession();
  const [settings, setSettings] = useState<UserSettingsResponse | null>(null);
  const [displayName, setDisplayName] = useState("");
  const [theme, setTheme] = useState("default");
  const [playbackSpeed, setPlaybackSpeed] = useState(1.0);
  const [subtitleLang, setSubtitleLang] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [localTheme, setLocalTheme] = useState(loadLocalTheme());

  const radiusValue = useMemo(() => localTheme.radiusBase, [localTheme.radiusBase]);

  useEffect(() => {
    if (!session) return;
    apiFetch<UserSettingsResponse>("/api/settings", {}, session.token)
      .then((data) => {
        setSettings(data);
        setDisplayName(data.display_name ?? "");
        setTheme(data.theme);
        setPlaybackSpeed(data.playback_speed);
        setSubtitleLang(data.subtitle_lang ?? "");
      })
      .catch((err) => {
        setError((err as Error).message || "Failed to load settings.");
      });
  }, [session]);

  useEffect(() => {
    applyLocalTheme(localTheme);
    saveLocalTheme(localTheme);
  }, [localTheme]);

  async function handleSave(event: React.FormEvent) {
    event.preventDefault();
    if (!session) return;
    setSaving(true);
    setError(null);
    setStatus(null);
    try {
      const payload: Record<string, unknown> = {
        theme: theme.trim() || "default",
        playback_speed: playbackSpeed
      };
      const display = displayName.trim();
      const subtitle = subtitleLang.trim();
      if (display) {
        payload.display_name = display;
      }
      if (subtitle) {
        payload.subtitle_lang = subtitle;
      }

      const updated = await apiFetch<UserSettingsResponse>(
        "/api/settings",
        {
          method: "PUT",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify(payload)
        },
        session.token
      );
      setSettings(updated);
      setStatus("Saved.");
    } catch (err) {
      setError((err as Error).message || "Failed to save settings.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <AppShell title="Settings" subtitle="Playback preferences and profile info.">
      {error ? <div className="settings-error">{error}</div> : null}
      {status ? <div className="settings-status">{status}</div> : null}

      <div className="settings-grid">
        <form className="settings-form app-card" onSubmit={handleSave}>
          <div className="app-card-header">
            <h2 className="app-card-title">Profile</h2>
            <p className="app-card-subtitle">Visible name and playback defaults.</p>
          </div>

          <label className="settings-label">
            <span>Display name</span>
            <input
              className="app-input"
              type="text"
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
              placeholder={settings?.display_name ?? "Your name"}
            />
          </label>

          <label className="settings-label">
            <span>Theme</span>
            <input
              className="app-input"
              type="text"
              value={theme}
              onChange={(event) => setTheme(event.target.value)}
            />
          </label>

          <label className="settings-label">
            <span>Playback speed</span>
            <input
              className="app-input"
              type="number"
              min="0.25"
              max="4"
              step="0.1"
              value={playbackSpeed}
              onChange={(event) => {
                const next = Number(event.target.value);
                if (!Number.isNaN(next)) {
                  setPlaybackSpeed(next);
                }
              }}
            />
          </label>

          <label className="settings-label">
            <span>Subtitle language</span>
            <input
              className="app-input"
              type="text"
              value={subtitleLang}
              onChange={(event) => setSubtitleLang(event.target.value)}
              placeholder="zh, en, ja..."
            />
          </label>

          <div className="settings-actions">
            <button type="submit" className="app-btn primary" disabled={saving}>
              {saving ? "Saving..." : "Save settings"}
            </button>
          </div>
        </form>

        <section className="settings-form app-card">
          <div className="app-card-header">
            <h2 className="app-card-title">Appearance (local)</h2>
            <p className="app-card-subtitle">Saved in this browser only.</p>
          </div>

          <div className="settings-color-grid">
            <label className="settings-label">
              <span>Accent</span>
              <input
                className="settings-color"
                type="color"
                value={localTheme.accent}
                onChange={(event) =>
                  setLocalTheme((prev) => ({ ...prev, accent: event.target.value }))
                }
              />
            </label>
            <label className="settings-label">
              <span>Accent 2</span>
              <input
                className="settings-color"
                type="color"
                value={localTheme.accent2}
                onChange={(event) =>
                  setLocalTheme((prev) => ({ ...prev, accent2: event.target.value }))
                }
              />
            </label>
            <label className="settings-label">
              <span>Text</span>
              <input
                className="settings-color"
                type="color"
                value={localTheme.ink}
                onChange={(event) =>
                  setLocalTheme((prev) => ({ ...prev, ink: event.target.value }))
                }
              />
            </label>
            <label className="settings-label">
              <span>Muted</span>
              <input
                className="settings-color"
                type="color"
                value={localTheme.muted}
                onChange={(event) =>
                  setLocalTheme((prev) => ({ ...prev, muted: event.target.value }))
                }
              />
            </label>
            <label className="settings-label">
              <span>Background</span>
              <input
                className="settings-color"
                type="color"
                value={localTheme.bgMain}
                onChange={(event) =>
                  setLocalTheme((prev) => ({ ...prev, bgMain: event.target.value }))
                }
              />
            </label>
            <label className="settings-label">
              <span>Surface</span>
              <input
                className="settings-color"
                type="color"
                value={localTheme.bgSoft}
                onChange={(event) =>
                  setLocalTheme((prev) => ({ ...prev, bgSoft: event.target.value }))
                }
              />
            </label>
          </div>

          <label className="settings-label">
            <span>Corner radius: {radiusValue}px</span>
            <input
              className="settings-range"
              type="range"
              min="6"
              max="24"
              step="1"
              value={localTheme.radiusBase}
              onChange={(event) =>
                setLocalTheme((prev) => ({
                  ...prev,
                  radiusBase: Number(event.target.value)
                }))
              }
            />
          </label>

          <div className="settings-actions">
            <button
              type="button"
              className="app-btn ghost"
              onClick={() => setLocalTheme(resetLocalTheme())}
            >
              Reset to default
            </button>
          </div>
        </section>
      </div>
    </AppShell>
  );
}
