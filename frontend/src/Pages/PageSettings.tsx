import { useEffect, useMemo, useRef, useState } from "react";
import AppShell from "../Components/AppShell";
import { apiFetch } from "../api";
import { useSession } from "../session";
import {
  applyLocalTheme,
  createPreset,
  loadLocalPresets,
  loadLocalTheme,
  resetLocalTheme,
  saveLocalPresets,
  saveLocalTheme,
  type ThemePreset
} from "../theme";
import { libraryColumnsRange, loadLibraryColumns, saveLibraryColumns } from "../uiSettings";
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
  const [presets, setPresets] = useState<ThemePreset[]>(loadLocalPresets());
  const [presetName, setPresetName] = useState("");
  const [libraryColumns, setLibraryColumns] = useState(loadLibraryColumns());
  const importInputRef = useRef<HTMLInputElement | null>(null);

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

  useEffect(() => {
    saveLocalPresets(presets);
  }, [presets]);

  useEffect(() => {
    saveLibraryColumns(libraryColumns);
  }, [libraryColumns]);

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

  function handleSavePreset() {
    const name = presetName.trim();
    if (!name) {
      setError("Preset name is required.");
      return;
    }
    setError(null);
    setStatus(null);

    const existing = presets.find((preset) => preset.name.toLowerCase() === name.toLowerCase());
    if (existing) {
      setPresets((prev) =>
        prev.map((preset) =>
          preset.id === existing.id ? { ...preset, settings: { ...localTheme } } : preset
        )
      );
      setStatus("Preset updated.");
      return;
    }

    const preset = createPreset(name, localTheme);
    setPresets((prev) => [...prev, preset]);
    setPresetName("");
    setStatus("Preset saved.");
  }

  function handleExportPresets() {
    const payload = JSON.stringify(presets, null, 2);
    const blob = new Blob([payload], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "anicargo-theme-presets.json";
    link.click();
    URL.revokeObjectURL(url);
  }

  async function handleImportFile(file: File) {
    try {
      const raw = await file.text();
      const parsed = JSON.parse(raw);
      const list = Array.isArray(parsed) ? parsed : parsed?.presets;
      if (!Array.isArray(list)) {
        setError("Invalid preset file.");
        return;
      }
      const imported = list
        .filter((item) => item && typeof item === "object" && item.settings)
        .map((item) => createPreset(item.name || "Imported", item.settings));
      if (!imported.length) {
        setError("No presets found in file.");
        return;
      }
      setPresets((prev) => [...prev, ...imported]);
      setStatus(`Imported ${imported.length} preset(s).`);
    } catch {
      setError("Failed to import preset file.");
    }
  }

  function handleImportPresets(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) return;
    handleImportFile(file);
    event.target.value = "";
  }

  function handleApplyPreset(preset: ThemePreset) {
    setLocalTheme(preset.settings);
    setStatus(`Applied preset: ${preset.name}`);
  }

  function handleDeletePreset(id: string) {
    setPresets((prev) => prev.filter((preset) => preset.id !== id));
    setStatus("Preset removed.");
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

          <label className="settings-label">
            <span>Library columns</span>
            <input
              className="app-input"
              type="number"
              min={libraryColumnsRange.min}
              max={libraryColumnsRange.max}
              value={libraryColumns}
              onChange={(event) => {
                const next = Number(event.target.value);
                if (!Number.isNaN(next)) {
                  setLibraryColumns(next);
                }
              }}
            />
          </label>

          <div className="settings-preset">
            <div className="settings-preset-row">
              <input
                className="app-input"
                type="text"
                placeholder="Preset name"
                value={presetName}
                onChange={(event) => setPresetName(event.target.value)}
              />
              <button type="button" className="app-btn" onClick={handleSavePreset}>
                Save preset
              </button>
              <button type="button" className="app-btn ghost" onClick={handleExportPresets}>
                Export
              </button>
              <button
                type="button"
                className="app-btn ghost"
                onClick={() => importInputRef.current?.click()}
              >
                Import
              </button>
              <button
                type="button"
                className="app-btn ghost"
                onClick={() => setLocalTheme(resetLocalTheme())}
              >
                Reset
              </button>
            </div>
            <input
              ref={importInputRef}
              type="file"
              accept="application/json"
              className="settings-hidden-input"
              onChange={handleImportPresets}
            />

            {presets.length ? (
              <div className="settings-preset-list">
                {presets.map((preset) => (
                  <div key={preset.id} className="settings-preset-item">
                    <span>{preset.name}</span>
                    <div className="settings-preset-actions">
                      <button
                        type="button"
                        className="app-btn ghost"
                        onClick={() => handleApplyPreset(preset)}
                      >
                        Apply
                      </button>
                      <button
                        type="button"
                        className="app-btn ghost"
                        onClick={() => handleDeletePreset(preset.id)}
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="app-muted">No presets saved.</div>
            )}
          </div>
        </section>
      </div>
    </AppShell>
  );
}
