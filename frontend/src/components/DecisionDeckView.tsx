import React, { useState, useEffect, useCallback } from 'react';
import {
  FileText,
  Image as ImageIcon,
  RotateCcw,
  Sparkles,
  ArrowRight,
  Maximize2,
  Calendar,
  Compass,
  Trash2,
  SkipForward
} from 'lucide-react';
import { getThumbnailUrl, getFullFileUrl, classifySingle, streamClassify, type MediaItem } from '../services/api';

export type DecisionType = 'DOCUMENT' | 'PHOTO' | 'TRASH' | 'SKIP';

interface Props {
  mixedItems: MediaItem[];
  backend?: 'ollama' | 'gemini';
  onComplete: (decisions: Record<string, { item: MediaItem; decision: DecisionType }>) => void;
  onBack: () => void;
}

export const DecisionDeckView: React.FC<Props> = ({ mixedItems, backend = 'ollama', onComplete, onBack }) => {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [decisions, setDecisions] = useState<Record<string, { item: MediaItem; decision: DecisionType }>>({});
  const [history, setHistory] = useState<string[]>([]);
  const [aiLoading, setAiLoading] = useState(false);
  const [isStreamingAi, setIsStreamingAi] = useState(false);
  const [streamCount, setStreamCount] = useState(0);
  const [aiCaptions, setAiCaptions] = useState<Record<string, { caption: string; category?: string; reason?: string }>>({});
  const [showFull, setShowFull] = useState(false);
  const [failedImgIds, setFailedImgIds] = useState<Set<string>>(new Set());

  const currentItem = mixedItems[currentIndex];
  const isImgError = currentItem ? failedImgIds.has(currentItem.id) : false;

  // Derive active caption directly during render (avoids set-state-in-effect React warning)
  const activeCaption = currentItem
    ? (aiCaptions[currentItem.id]?.caption ||
       aiCaptions[currentItem.path]?.caption ||
       currentItem.caption ||
       currentItem.reason ||
       'Visual inspection needed')
    : '';

  const handleDecision = useCallback(
    (choice: DecisionType) => {
      if (!currentItem) return;
      setDecisions((prev) => ({
        ...prev,
        [currentItem.id]: { item: currentItem, decision: choice },
      }));
      setHistory((prev) => [...prev, currentItem.id]);

      if (currentIndex < mixedItems.length - 1) {
        setCurrentIndex((prev) => prev + 1);
      }
    },
    [currentItem, currentIndex, mixedItems.length]
  );

  const handleUndo = useCallback(() => {
    if (history.length === 0) return;
    const lastId = history[history.length - 1];
    setDecisions((prev) => {
      const next = { ...prev };
      delete next[lastId];
      return next;
    });
    setHistory((prev) => prev.slice(0, -1));
    const targetIdx = mixedItems.findIndex((it) => it.id === lastId);
    if (targetIdx !== -1) setCurrentIndex(targetIdx);
  }, [history, mixedItems]);

  const handleAskMoondream = async () => {
    if (!currentItem) return;
    setAiLoading(true);
    try {
      const res = await classifySingle(currentItem.path, backend, 'moondream');
      const text = res.caption || res.reason;
      setAiCaptions((prev) => ({
        ...prev,
        [currentItem.id]: { caption: text, category: res.category, reason: res.reason },
        [currentItem.path]: { caption: text, category: res.category, reason: res.reason },
      }));
    } catch {
      setAiCaptions((prev) => ({
        ...prev,
        [currentItem.id]: { caption: `${backend === 'gemini' ? 'Gemini' : 'Local Ollama'} call failed` },
      }));
    } finally {
      setAiLoading(false);
    }
  };

  const handleStreamAllAi = async () => {
    if (isStreamingAi || mixedItems.length === 0) return;
    setIsStreamingAi(true);
    setStreamCount(0);
    try {
      const unanalyzed = mixedItems.filter((it) => !it.caption && !aiCaptions[it.id]?.caption);
      const paths = unanalyzed.map((it) => it.path);
      if (paths.length === 0) {
        setIsStreamingAi(false);
        return;
      }
      await streamClassify(
        paths,
        (data) => {
          setAiCaptions((prev) => {
            const next = {
              ...prev,
              [data.path]: { caption: data.caption || data.reason, category: data.category, reason: data.reason },
            };
            const match = mixedItems.find((it) => it.path === data.path);
            if (match) {
              next[match.id] = { caption: data.caption || data.reason, category: data.category, reason: data.reason };
            }
            return next;
          });
          setStreamCount((c) => c + 1);
        },
        backend,
        'moondream'
      );
    } catch (err) {
      console.error('Streaming AI failed:', err);
    } finally {
      setIsStreamingAi(false);
    }
  };

  // Keyboard shortcut listener
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      if (e.key === 'd' || e.key === 'D' || e.key === 'ArrowLeft') {
        e.preventDefault();
        handleDecision('DOCUMENT');
      } else if (e.key === 'f' || e.key === 'F' || e.key === 'ArrowRight') {
        e.preventDefault();
        handleDecision('PHOTO');
      } else if (e.key === 'x' || e.key === 'X' || e.key === 'Delete') {
        e.preventDefault();
        handleDecision('TRASH');
      } else if (e.key === 's' || e.key === 'S' || e.key === 'ArrowDown') {
        e.preventDefault();
        handleDecision('SKIP');
      } else if (e.key === 'z' || e.key === 'Z') {
        e.preventDefault();
        handleUndo();
      } else if (e.key === ' ') {
        e.preventDefault();
        setShowFull((prev) => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleDecision, handleUndo]);

  const decidedCount = Object.keys(decisions).length;
  const progressPercent = Math.round((decidedCount / mixedItems.length) * 100) || 0;

  if (!currentItem || decidedCount === mixedItems.length) {
    return (
      <div className="flex flex-col items-center justify-center h-[70vh] space-y-6 text-center">
        <div className="w-20 h-20 rounded-full bg-emerald-500/20 border border-emerald-500/40 flex items-center justify-center text-emerald-400 text-3xl">
          ✨
        </div>
        <h2 className="text-2xl font-bold text-white">All Ambiguous Photos Decided!</h2>
        <p className="text-zinc-400 max-w-md">
          You have reviewed all {mixedItems.length} mixed items. Ready to inspect your event clusters.
        </p>
        <button
          onClick={() => onComplete(decisions)}
          className="flex items-center gap-2 bg-emerald-600 hover:bg-emerald-500 text-white font-medium px-6 py-3 rounded-xl shadow-lg transition active:scale-95 cursor-pointer"
        >
          <span>Continue to Event Clusters</span>
          <ArrowRight className="w-5 h-5" />
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full space-y-4">
      {/* Top Status Bar */}
      <div className="flex items-center justify-between bg-zinc-900/80 border border-zinc-800 px-5 py-3 rounded-2xl">
        <div className="flex items-center gap-3">
          <button
            onClick={onBack}
            className="text-xs text-zinc-400 hover:text-white px-3 py-1.5 rounded-lg border border-zinc-800 hover:bg-zinc-800 cursor-pointer"
          >
            ← Clean Sweep
          </button>
          <span className="text-sm font-semibold text-white">
            Decision Deck ({currentIndex + 1} of {mixedItems.length})
          </span>
        </div>

        {/* Progress bar */}
        <div className="flex items-center gap-4 w-72">
          <div className="w-full bg-zinc-800 h-2 rounded-full overflow-hidden">
            <div
              className="bg-cyan-500 h-full transition-all duration-300"
              style={{ width: `${progressPercent}%` }}
            />
          </div>
          <span className="text-xs text-zinc-400 whitespace-nowrap font-mono">{progressPercent}%</span>
        </div>

        <div className="flex items-center gap-3">
          <button
            onClick={handleStreamAllAi}
            disabled={isStreamingAi}
            className={`flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-lg border transition ${
              isStreamingAi
                ? 'bg-purple-500/20 text-purple-300 border-purple-500/40 animate-pulse cursor-wait'
                : 'text-purple-300 border-purple-500/30 hover:bg-purple-500/10 cursor-pointer'
            }`}
            title="Stream AI Vision analysis asynchronously for all ambiguous items"
          >
            <Sparkles className="w-3.5 h-3.5 text-purple-400" />
            <span>
              {isStreamingAi
                ? `Analyzing (${streamCount}/${mixedItems.length})...`
                : 'Stream All AI'}
            </span>
          </button>

          <button
            onClick={handleUndo}
            disabled={history.length === 0}
            className={`flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-lg border transition ${
              history.length > 0
                ? 'text-zinc-300 border-zinc-700 hover:bg-zinc-800 cursor-pointer'
                : 'text-zinc-600 border-zinc-900 cursor-not-allowed'
            }`}
          >
            <RotateCcw className="w-3.5 h-3.5" />
            <span>Undo (Z)</span>
          </button>
        </div>
      </div>

      {/* Main Focus Viewport */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 flex-1 max-h-[64vh]">
        {/* Large Media Display */}
        <div className="lg:col-span-8 bg-zinc-950/80 border border-zinc-800/80 rounded-2xl flex items-center justify-center p-4 relative overflow-hidden group">
          {isImgError ? (
            <div className="flex flex-col items-center justify-center p-8 text-zinc-500">
              <ImageIcon className="w-16 h-16 mb-2 opacity-40 text-zinc-400" />
              <span className="text-sm font-medium text-zinc-300">{currentItem.name}</span>
              <span className="text-xs text-zinc-500 mt-1">Preview could not be loaded</span>
            </div>
          ) : (
            <img
              src={getThumbnailUrl(currentItem.path, 1024)}
              alt={currentItem.name}
              onError={() => {
                if (currentItem) {
                  setFailedImgIds((prev) => new Set(prev).add(currentItem.id));
                }
              }}
              className="max-h-[58vh] max-w-full object-contain rounded-lg shadow-2xl transition duration-200"
            />
          )}

          <button
            onClick={() => setShowFull(!showFull)}
            className="absolute bottom-4 right-4 bg-zinc-900/90 hover:bg-zinc-800 text-zinc-300 p-2.5 rounded-xl border border-zinc-700 shadow-xl opacity-0 group-hover:opacity-100 transition cursor-pointer"
            title="Inspect Full Resolution (Space)"
          >
            <Maximize2 className="w-4 h-4" />
          </button>
        </div>

        {/* Sidebar Info & Decision Cockpit */}
        <div className="lg:col-span-4 flex flex-col justify-between space-y-4">
          {/* Metadata Card */}
          <div className="bg-zinc-900/70 border border-zinc-800/80 rounded-2xl p-5 space-y-4">
            <div>
              <div className="text-xs text-zinc-500 uppercase tracking-wider font-semibold">Media Item</div>
              <h3 className="text-sm font-bold text-zinc-100 break-all">{currentItem.name}</h3>
            </div>

            <div className="grid grid-cols-2 gap-3 text-xs text-zinc-400">
              <div className="flex items-center gap-2 bg-zinc-950/50 p-2 rounded-lg border border-zinc-800/60">
                <Calendar className="w-4 h-4 text-cyan-400 shrink-0" />
                <span className="truncate">{currentItem.date_str}</span>
              </div>
              <div className="flex items-center gap-2 bg-zinc-950/50 p-2 rounded-lg border border-zinc-800/60">
                <Compass className="w-4 h-4 text-amber-400 shrink-0" />
                <span className="truncate">{currentItem.has_gps ? 'GPS Tagged' : 'No GPS'}</span>
              </div>
            </div>

            {/* Moondream Vision Insight */}
            <div className="p-3.5 bg-zinc-950/80 border border-zinc-800 rounded-xl space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-purple-400 flex items-center gap-1.5">
                  <Sparkles className="w-3.5 h-3.5" />
                  {backend === 'gemini' ? 'Gemini Vision' : 'Moondream Vision'}
                </span>
                <div className="flex items-center gap-1.5">
                  {(aiCaptions[currentItem.id]?.category || currentItem.category) && (
                    <span
                      className={`text-[10px] font-mono px-1.5 py-0.5 rounded border ${
                        (aiCaptions[currentItem.id]?.category || currentItem.category) === 'DOCUMENT'
                          ? 'bg-amber-500/10 text-amber-300 border-amber-500/30'
                          : 'bg-cyan-500/10 text-cyan-300 border-cyan-500/30'
                      }`}
                    >
                      {aiCaptions[currentItem.id]?.category || currentItem.category}
                    </span>
                  )}
                  <button
                    onClick={handleAskMoondream}
                    disabled={aiLoading}
                    className="text-[10px] text-zinc-400 hover:text-white border border-zinc-800 px-2 py-0.5 rounded transition cursor-pointer"
                  >
                    {aiLoading ? 'Analyzing...' : 'Re-scan'}
                  </button>
                </div>
              </div>
              <p className="text-xs text-zinc-300 leading-relaxed italic">
                "{activeCaption || 'Analyzing scene...'}"
              </p>
            </div>
          </div>

          {/* Action Buttons */}
          <div className="space-y-2.5">
            <button
              onClick={() => handleDecision('DOCUMENT')}
              className="w-full flex items-center justify-between p-3.5 rounded-xl border border-amber-500/30 bg-amber-500/10 hover:bg-amber-500/20 text-amber-200 font-semibold transition active:scale-98 cursor-pointer shadow-lg"
            >
              <div className="flex items-center gap-3">
                <FileText className="w-5 h-5 text-amber-400" />
                <span>Move to Pictures_Doc</span>
              </div>
              <kbd className="bg-amber-950/80 border border-amber-500/40 text-amber-300 text-xs px-2.5 py-1 rounded font-mono font-bold">
                D
              </kbd>
            </button>

            <button
              onClick={() => handleDecision('PHOTO')}
              className="w-full flex items-center justify-between p-3.5 rounded-xl border border-cyan-500/30 bg-cyan-500/10 hover:bg-cyan-500/20 text-cyan-200 font-semibold transition active:scale-98 cursor-pointer shadow-lg"
            >
              <div className="flex items-center gap-3">
                <ImageIcon className="w-5 h-5 text-cyan-400" />
                <span>Keep in Family Pictures</span>
              </div>
              <kbd className="bg-cyan-950/80 border border-cyan-500/40 text-cyan-300 text-xs px-2.5 py-1 rounded font-mono font-bold">
                F
              </kbd>
            </button>

            <div className="grid grid-cols-2 gap-2.5 pt-1">
              <button
                onClick={() => handleDecision('TRASH')}
                className="flex items-center justify-between p-3 rounded-xl border border-rose-500/30 bg-rose-500/10 hover:bg-rose-500/20 text-rose-200 font-medium text-xs transition active:scale-98 cursor-pointer shadow"
              >
                <div className="flex items-center gap-2">
                  <Trash2 className="w-4 h-4 text-rose-400 shrink-0" />
                  <span>Trash</span>
                </div>
                <kbd className="bg-rose-950/80 border border-rose-500/40 text-rose-300 text-[11px] px-2 py-0.5 rounded font-mono font-bold">
                  X
                </kbd>
              </button>

              <button
                onClick={() => handleDecision('SKIP')}
                className="flex items-center justify-between p-3 rounded-xl border border-zinc-700 bg-zinc-800/60 hover:bg-zinc-800 text-zinc-300 font-medium text-xs transition active:scale-98 cursor-pointer shadow"
              >
                <div className="flex items-center gap-2">
                  <SkipForward className="w-4 h-4 text-zinc-400 shrink-0" />
                  <span>Skip</span>
                </div>
                <kbd className="bg-zinc-900 border border-zinc-700 text-zinc-300 text-[11px] px-2 py-0.5 rounded font-mono font-bold">
                  S
                </kbd>
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Fullscreen Modal View */}
      {showFull && (
        <div
          className="fixed inset-0 z-50 bg-black/90 flex items-center justify-center p-6 cursor-pointer"
          onClick={() => setShowFull(false)}
        >
          {currentItem.is_video ? (
            <video
              src={getFullFileUrl(currentItem.path)}
              controls
              autoPlay
              onClick={(event) => event.stopPropagation()}
              className="max-h-full max-w-full rounded-lg shadow-2xl"
            />
          ) : (
            <img
              src={getFullFileUrl(currentItem.path)}
              alt={currentItem.name}
              className="max-h-full max-w-full object-contain rounded-lg shadow-2xl"
            />
          )}
        </div>
      )}
    </div>
  );
};
