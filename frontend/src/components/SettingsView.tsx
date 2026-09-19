import { useState } from 'react';
import { FolderOpen, Save, X } from 'lucide-react';
import { type AppSettings, pickFolder, saveSettings } from '../services/api';

interface SettingsViewProps {
  settings: AppSettings;
  onSaved: (settings: AppSettings) => void;
  onClose: () => void;
}

type FolderKey = 'source_dir' | 'pictures_dir' | 'videos_dir';

export function SettingsView({ settings, onSaved, onClose }: SettingsViewProps) {
  const [draft, setDraft] = useState<AppSettings>(settings);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const updatePath = (key: FolderKey, value: string) => {
    setDraft((current) => ({ ...current, [key]: value }));
  };

  const chooseFolder = async (key: FolderKey, title: string) => {
    const selected = await pickFolder(title);
    if (selected) updatePath(key, selected);
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      const saved = await saveSettings(draft);
      onSaved(saved);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not save settings');
    } finally {
      setSaving(false);
    }
  };

  const folderField = (key: FolderKey, label: string, description: string, optional = false) => {
    const value = draft[key] || '';
    return (
      <div className="space-y-2">
        <div>
          <label htmlFor={key} className="text-sm font-semibold text-zinc-200">
            {label} {optional && <span className="text-xs font-normal text-zinc-500">(optional)</span>}
          </label>
          <p className="text-xs text-zinc-500 mt-1">{description}</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-2 flex-1 bg-zinc-950 border border-zinc-800 rounded-xl p-2.5 focus-within:border-cyan-500 transition">
            <FolderOpen className="w-4 h-4 text-zinc-500 shrink-0" />
            <input
              id={key}
              value={value}
              onChange={(event) => updatePath(key, event.target.value)}
              className="bg-transparent w-full text-sm text-white focus:outline-none"
              placeholder={optional ? 'Leave empty to configure later' : 'Enter an absolute folder path'}
            />
          </div>
          <button
            type="button"
            onClick={() => chooseFolder(key, `Choose ${label}`)}
            className="px-3 py-2.5 rounded-xl border border-zinc-700 bg-zinc-800 hover:bg-zinc-700 text-xs text-zinc-200 transition"
          >
            Browse
          </button>
          {optional && value && (
            <button
              type="button"
              onClick={() => updatePath(key, '')}
              className="p-2.5 rounded-xl border border-zinc-800 text-zinc-500 hover:text-zinc-200 transition"
              title={`Clear ${label}`}
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>
    );
  };

  return (
    <section className="max-w-3xl mx-auto w-full space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs uppercase tracking-widest text-cyan-400 font-semibold">Configuration</p>
          <h2 className="text-3xl font-extrabold text-white mt-1">Folder Settings</h2>
          <p className="text-sm text-zinc-400 mt-2">Choose where DreamCatcher reads and organizes your media.</p>
        </div>
        <button type="button" onClick={onClose} className="p-2 rounded-xl text-zinc-400 hover:text-white hover:bg-zinc-800 transition">
          <X className="w-5 h-5" />
        </button>
      </div>

      <div className="space-y-6 bg-zinc-900/60 p-6 rounded-3xl border border-zinc-800/80">
        {folderField('source_dir', 'Source folder', 'The folder scanned during the ingest step.')}
        <div className="border-t border-zinc-800" />
        {folderField('pictures_dir', 'Pictures destination', 'Optional destination for picture albums.', true)}
        {folderField('videos_dir', 'Videos destination', 'Optional destination for video albums.', true)}
        <p className="text-xs text-zinc-500">Destinations must already exist, be readable, and must not overlap. Saving settings never moves files.</p>
      </div>

      {error && <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-sm text-rose-300">{error}</div>}

      <div className="flex justify-end gap-3">
        <button type="button" onClick={onClose} className="px-4 py-2.5 rounded-xl text-sm text-zinc-400 hover:text-white transition">Cancel</button>
        <button type="button" onClick={handleSave} disabled={saving} className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 text-sm font-semibold transition">
          <Save className="w-4 h-4" />
          {saving ? 'Saving…' : 'Save settings'}
        </button>
      </div>
    </section>
  );
}
