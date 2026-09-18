import React, { useState } from 'react';
import { Folder, Edit2, Check, Sparkles, Calendar, ArrowRight } from 'lucide-react';
import { getThumbnailUrl, type Cluster } from '../services/api';

interface Props {
  clusters: Cluster[];
  onCommit: (customNames: Record<string, string>) => void;
  onBack: () => void;
}

export const ClusterView: React.FC<Props> = ({ clusters, onCommit, onBack }) => {
  const [folderNames, setFolderNames] = useState<Record<string, string>>(() => {
    const init: Record<string, string> = {};
    clusters.forEach((c) => {
      init[c.id] = c.folder_name;
    });
    return init;
  });
  const [editingId, setEditingId] = useState<string | null>(null);

  const handleNameChange = (id: string, newName: string) => {
    setFolderNames((prev) => ({ ...prev, [id]: newName }));
  };

  const totalPhotos = clusters.reduce((acc, c) => acc + c.items.length, 0);

  return (
    <div className="flex flex-col h-full space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between bg-zinc-900/80 border border-zinc-800 p-5 rounded-2xl">
        <div className="flex items-center space-x-4">
          <div className="p-3 bg-cyan-500/10 border border-cyan-500/20 rounded-xl text-cyan-400">
            <Folder className="w-6 h-6" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white tracking-wide flex items-center gap-2">
              Phase 3: Event Clusters & Archive Studio
            </h2>
            <p className="text-sm text-zinc-400">
              {clusters.length} event/daily folders generated from {totalPhotos} family memories. Click to rename any folder.
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <button
            onClick={onBack}
            className="text-xs text-zinc-400 hover:text-white px-3 py-2 rounded-xl border border-zinc-800 hover:bg-zinc-800 cursor-pointer"
          >
            ← Back
          </button>

          <button
            onClick={() => onCommit(folderNames)}
            className="flex items-center gap-2 bg-emerald-600 hover:bg-emerald-500 text-white font-medium px-5 py-2.5 rounded-xl shadow-lg transition active:scale-95 cursor-pointer"
          >
            <span>Review Final Move</span>
            <ArrowRight className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Cluster Cards Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 overflow-y-auto pr-2 max-h-[64vh]">
        {clusters.map((c) => {
          const isEditing = editingId === c.id;
          const currentName = folderNames[c.id] || c.folder_name;

          return (
            <div
              key={c.id}
              className={`p-4 rounded-2xl border transition ${
                c.is_holiday
                  ? 'border-purple-500/40 bg-purple-950/10'
                  : c.is_event
                  ? 'border-cyan-500/30 bg-zinc-900/60'
                  : 'border-zinc-800 bg-zinc-950/50'
              }`}
            >
              {/* Folder Title / Edit Field */}
              <div className="flex items-center justify-between gap-2 mb-3">
                {isEditing ? (
                  <div className="flex items-center gap-2 w-full">
                    <input
                      type="text"
                      value={currentName}
                      onChange={(e) => handleNameChange(c.id, e.target.value)}
                      className="bg-zinc-950 border border-cyan-500 rounded-lg px-2.5 py-1 text-sm text-white w-full focus:outline-none"
                      autoFocus
                    />
                    <button
                      onClick={() => setEditingId(null)}
                      className="p-1.5 bg-cyan-600 text-white rounded-lg hover:bg-cyan-500 cursor-pointer shrink-0"
                    >
                      <Check className="w-4 h-4" />
                    </button>
                  </div>
                ) : (
                  <div className="flex items-center justify-between w-full">
                    <div className="flex items-center gap-2 truncate">
                      {c.is_holiday && <Sparkles className="w-4 h-4 text-purple-400 shrink-0" />}
                      <span className="text-sm font-semibold text-zinc-100 truncate" title={currentName}>
                        {currentName}
                      </span>
                    </div>
                    <button
                      onClick={() => setEditingId(c.id)}
                      className="text-zinc-500 hover:text-zinc-300 p-1 rounded-md hover:bg-zinc-800/60 transition cursor-pointer shrink-0"
                      title="Rename Folder"
                    >
                      <Edit2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                )}
              </div>

              {/* Subtitle / Meta */}
              <div className="flex items-center gap-2 text-xs text-zinc-400 mb-3">
                <Calendar className="w-3.5 h-3.5 text-zinc-500" />
                <span>{c.date_str}</span>
                <span>•</span>
                <span className="text-zinc-300 font-mono">{c.items.length} items</span>
              </div>

              {/* Thumbnail Strip */}
              <div className="flex items-center gap-1.5 overflow-x-auto py-1">
                {c.items.slice(0, 5).map((it) => (
                  <div
                    key={it.id}
                    className="w-14 h-14 shrink-0 rounded-lg overflow-hidden bg-zinc-900 border border-zinc-800/80"
                  >
                    <img
                      src={getThumbnailUrl(it.path, 120)}
                      alt={it.name}
                      className="w-full h-full object-cover"
                      loading="lazy"
                    />
                  </div>
                ))}
                {c.items.length > 5 && (
                  <div className="w-14 h-14 shrink-0 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center text-xs text-zinc-400 font-semibold">
                    +{c.items.length - 5}
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
