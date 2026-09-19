import React, { useState } from 'react';
import { CheckCircle, FileText, Image as ImageIcon, ArrowRight, ShieldCheck, Trash2 } from 'lucide-react';
import { getThumbnailUrl, type MediaItem } from '../services/api';

interface Props {
  obviousDocs: MediaItem[];
  obviousPhotos: MediaItem[];
  onApprove: (
    approvedDocs: MediaItem[],
    approvedPhotos: MediaItem[],
    demotedToMixed: MediaItem[],
    trashItems?: MediaItem[]
  ) => void;
}

export const CleanSweepView: React.FC<Props> = ({ obviousDocs, obviousPhotos, onApprove }) => {
  const [selectedTab, setSelectedTab] = useState<'docs' | 'photos'>('docs');
  const [excludedIds, setExcludedIds] = useState<Set<string>>(new Set());
  const [trashIds, setTrashIds] = useState<Set<string>>(new Set());

  const toggleExclude = (id: string) => {
    // If it was marked as trash, unmark trash first
    if (trashIds.has(id)) {
      setTrashIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
      return;
    }

    setExcludedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleTrash = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    setTrashIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
        // Remove from excluded if present
        setExcludedIds((ex) => {
          const exNext = new Set(ex);
          exNext.delete(id);
          return exNext;
        });
      }
      return next;
    });
  };

  const handleApproveAll = () => {
    const demoted: MediaItem[] = [];
    const trashItems: MediaItem[] = [];

    const validDocs = obviousDocs.filter((d) => {
      if (trashIds.has(d.id)) {
        trashItems.push(d);
        return false;
      }
      if (excludedIds.has(d.id)) {
        demoted.push(d);
        return false;
      }
      return true;
    });

    const validPhotos = obviousPhotos.filter((p) => {
      if (trashIds.has(p.id)) {
        trashItems.push(p);
        return false;
      }
      if (excludedIds.has(p.id)) {
        demoted.push(p);
        return false;
      }
      return true;
    });

    onApprove(validDocs, validPhotos, demoted, trashItems);
  };

  const currentList = selectedTab === 'docs' ? obviousDocs : obviousPhotos;
  const totalApproved = obviousDocs.length + obviousPhotos.length - excludedIds.size - trashIds.size;

  return (
    <div className="flex flex-col h-full space-y-6">
      {/* Header Banner */}
      <div className="flex items-center justify-between bg-zinc-900/80 border border-zinc-800 p-5 rounded-2xl backdrop-blur-md">
        <div className="flex items-center space-x-4">
          <div className="p-3 bg-emerald-500/10 border border-emerald-500/20 rounded-xl text-emerald-400">
            <ShieldCheck className="w-6 h-6" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-white tracking-wide flex items-center gap-2">
              Phase 1: The Clean Sweep
            </h2>
            <p className="text-sm text-zinc-400">
              High-confidence items detected by rules and heuristics. Review the batch and approve in one click.
            </p>
          </div>
        </div>

        <button
          onClick={handleApproveAll}
          className="flex items-center gap-2 bg-emerald-600 hover:bg-emerald-500 text-white font-medium px-5 py-2.5 rounded-xl shadow-lg shadow-emerald-950 transition active:scale-95 cursor-pointer"
        >
          <span>
            Approve {totalApproved} Items
            {trashIds.size > 0 && ` (${trashIds.size} Trash)`}
          </span>
          <ArrowRight className="w-4 h-4" />
        </button>
      </div>

      {/* Tabs & Stats */}
      <div className="flex items-center justify-between border-b border-zinc-800 pb-3">
        <div className="flex items-center gap-3">
          <button
            onClick={() => setSelectedTab('docs')}
            className={`flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-medium transition cursor-pointer ${
              selectedTab === 'docs'
                ? 'bg-amber-500/20 text-amber-300 border border-amber-500/30'
                : 'text-zinc-400 hover:bg-zinc-800/60'
            }`}
          >
            <FileText className="w-4 h-4" />
            <span>Documents & Receipts ({obviousDocs.length})</span>
          </button>

          <button
            onClick={() => setSelectedTab('photos')}
            className={`flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-medium transition cursor-pointer ${
              selectedTab === 'photos'
                ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/30'
                : 'text-zinc-400 hover:bg-zinc-800/60'
            }`}
          >
            <ImageIcon className="w-4 h-4" />
            <span>Family Memories ({obviousPhotos.length})</span>
          </button>
        </div>

        <span className="text-xs text-zinc-500">
          Click card to exclude (send to Deck) • Click trash icon to mark trash
        </span>
      </div>

      {/* Image Grid */}
      <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8 gap-3 overflow-y-auto pr-2 flex-1 max-h-[62vh]">
        {currentList.map((item) => {
          const isTrash = trashIds.has(item.id);
          const isExcluded = excludedIds.has(item.id);

          return (
            <div
              key={item.id}
              onClick={() => toggleExclude(item.id)}
              className={`group relative rounded-xl overflow-hidden border cursor-pointer transition select-none ${
                isTrash
                  ? 'border-rose-500/60 bg-rose-950/20'
                  : isExcluded
                  ? 'border-zinc-800 bg-zinc-950/40 opacity-40 grayscale'
                  : selectedTab === 'docs'
                  ? 'border-amber-500/40 hover:border-amber-400'
                  : 'border-cyan-500/40 hover:border-cyan-400'
              }`}
            >
              <div className="aspect-square bg-zinc-900 relative overflow-hidden">
                <img
                  src={getThumbnailUrl(item.path, 240)}
                  alt={item.name}
                  loading="lazy"
                  className="w-full h-full object-cover group-hover:scale-105 transition duration-200"
                />

                {/* Quick Trash Button */}
                <button
                  type="button"
                  onClick={(e) => toggleTrash(e, item.id)}
                  className={`absolute top-1.5 left-1.5 p-1 rounded-md transition ${
                    isTrash
                      ? 'bg-rose-600 text-white'
                      : 'bg-zinc-900/80 text-zinc-400 hover:text-rose-400 hover:bg-zinc-900 opacity-0 group-hover:opacity-100'
                  }`}
                  title={isTrash ? 'Unmark Trash' : 'Mark as Trash'}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>

                {/* Check / Status Icon */}
                <div className="absolute top-1.5 right-1.5">
                  <CheckCircle
                    className={`w-5 h-5 drop-shadow-md ${
                      isTrash
                        ? 'text-rose-400 fill-rose-950'
                        : isExcluded
                        ? 'text-zinc-600'
                        : 'text-emerald-400 fill-emerald-950'
                    }`}
                  />
                </div>
              </div>

              <div className="p-2 bg-zinc-900/90 text-left">
                <div className="text-xs font-semibold text-zinc-200 truncate" title={item.name}>
                  {item.name}
                </div>
                <div className="text-[10px] text-zinc-400 truncate" title={item.reason}>
                  {isTrash ? 'Marked as Trash' : item.reason}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

