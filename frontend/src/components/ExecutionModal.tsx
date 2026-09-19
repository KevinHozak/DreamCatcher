import React, { useState } from 'react';
import { CheckCircle, AlertTriangle, Play, RefreshCw, X, HardDrive, Trash2, SkipForward } from 'lucide-react';
import { executeTriage } from '../services/api';

interface Props {
  sourceDir: string;
  picturesDir?: string | null;
  videosDir?: string | null;
  decisions: Record<string, any>;
  onClose: () => void;
  onSuccess: () => void;
}

export const ExecutionModal: React.FC<Props> = ({
  sourceDir,
  picturesDir,
  videosDir,
  decisions,
  onClose,
  onSuccess,
}) => {
  const [action, setAction] = useState<'move' | 'copy'>('move');
  const [isExecuting, setIsExecuting] = useState(false);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const decisionList = Object.values(decisions);
  const docCount = decisionList.filter((d: any) => d.category === 'DOCUMENT').length;
  const photoCount = decisionList.filter((d: any) => d.category === 'PHOTO' && !d.is_video).length;
  const videoCount = decisionList.filter((d: any) => d.category === 'PHOTO' && d.is_video).length;
  const trashCount = decisionList.filter((d: any) => d.category === 'TRASH').length;
  const skipCount = decisionList.filter((d: any) => d.category === 'SKIP').length;
  const sidecarCount = decisionList.filter((d: any) => d.has_sidecar && d.category !== 'SKIP').length;

  const handleRunExecution = async () => {
    setIsExecuting(true);
    setError(null);
    try {
      const res = await executeTriage(sourceDir, decisions, action, picturesDir, videosDir);
      setResult(res);
      onSuccess();
    } catch (err: any) {
      setError(err.message || 'Execution failed');
    } finally {
      setIsExecuting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-zinc-900 border border-zinc-800 rounded-3xl max-w-xl w-full p-6 space-y-6 shadow-2xl relative">
        {/* Close Button */}
        {!isExecuting && (
          <button
            onClick={onClose}
            className="absolute top-5 right-5 text-zinc-400 hover:text-white cursor-pointer"
          >
            <X className="w-5 h-5" />
          </button>
        )}

        {/* Title */}
        <div className="flex items-center gap-3">
          <div className="p-3 bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 rounded-xl">
            <HardDrive className="w-6 h-6" />
          </div>
          <div>
            <h3 className="text-xl font-bold text-white">Execute Triage Plan</h3>
            <p className="text-xs text-zinc-400">Review final counts before moving files on disk</p>
          </div>
        </div>

        {/* Summary Breakdown */}
        <div className="grid grid-cols-2 gap-3 text-sm">
          <div className="bg-zinc-950 p-3.5 rounded-xl border border-zinc-800">
            <div className="text-xs text-zinc-500 font-semibold">Family Pictures</div>
            <div className="text-lg font-bold text-cyan-400">{photoCount} photos</div>
            <div className="text-[11px] text-zinc-400 truncate" title={picturesDir || `${sourceDir}\\Pictures`}>→ {picturesDir || `${sourceDir}\\Pictures`} (Event & Daily)</div>
          </div>

          <div className="bg-zinc-950 p-3.5 rounded-xl border border-zinc-800">
            <div className="text-xs text-zinc-500 font-semibold">Family Videos</div>
            <div className="text-lg font-bold text-cyan-400">{videoCount} videos</div>
            <div className="text-[11px] text-zinc-400 truncate" title={videosDir || `${sourceDir}\\Videos`}>→ {videosDir || `${sourceDir}\\Videos`}</div>
          </div>

          <div className="bg-zinc-950 p-3.5 rounded-xl border border-zinc-800">
            <div className="text-xs text-zinc-500 font-semibold">Documents & Receipts</div>
            <div className="text-lg font-bold text-amber-400">{docCount} files</div>
            <div className="text-[11px] text-zinc-400">→ Pictures_Doc/ (Monthly)</div>
          </div>

          {trashCount > 0 && (
            <div className="bg-zinc-950 p-3.5 rounded-xl border border-rose-950/60">
              <div className="text-xs text-rose-400/80 font-semibold flex items-center gap-1">
                <Trash2 className="w-3 h-3" />
                <span>Utility Trash</span>
              </div>
              <div className="text-lg font-bold text-rose-400">{trashCount} files</div>
              <div className="text-[11px] text-zinc-400">→ Trash/ (Safely isolated)</div>
            </div>
          )}

          {skipCount > 0 && (
            <div className="bg-zinc-950 p-3.5 rounded-xl border border-zinc-800">
              <div className="text-xs text-zinc-400 font-semibold flex items-center gap-1">
                <SkipForward className="w-3 h-3" />
                <span>Skipped / In Place</span>
              </div>
              <div className="text-lg font-bold text-zinc-300">{skipCount} files</div>
              <div className="text-[11px] text-zinc-400">Left untouched in folder</div>
            </div>
          )}

          <div className="bg-zinc-950 p-3.5 rounded-xl border border-zinc-800">
            <div className="text-xs text-zinc-500 font-semibold">Sidecars Paired</div>
            <div className="text-lg font-bold text-emerald-400">{sidecarCount} sidecars</div>
            <div className="text-[11px] text-zinc-400">Moved alongside media</div>
          </div>
        </div>

        {/* Action Toggle */}
        <div className="space-y-2">
          <label className="text-xs font-semibold text-zinc-400">Operation Type</label>
          <div className="grid grid-cols-2 gap-3">
            <button
              type="button"
              onClick={() => setAction('move')}
              className={`p-3 rounded-xl border text-sm font-medium transition cursor-pointer ${
                action === 'move'
                  ? 'bg-emerald-500/20 text-emerald-300 border-emerald-500/40'
                  : 'bg-zinc-950 border-zinc-800 text-zinc-400'
              }`}
            >
              Move In-Place (Fast)
            </button>
            <button
              type="button"
              onClick={() => setAction('copy')}
              className={`p-3 rounded-xl border text-sm font-medium transition cursor-pointer ${
                action === 'copy'
                  ? 'bg-emerald-500/20 text-emerald-300 border-emerald-500/40'
                  : 'bg-zinc-950 border-zinc-800 text-zinc-400'
              }`}
            >
              Copy (Keep Originals)
            </button>
          </div>
        </div>

        {error && (
          <div className="flex items-center gap-2 p-3 bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs rounded-xl">
            <AlertTriangle className="w-4 h-4 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        {result ? (
          <div className="flex flex-col items-center space-y-3 p-4 bg-emerald-500/10 border border-emerald-500/20 rounded-2xl text-center">
            <CheckCircle className="w-8 h-8 text-emerald-400" />
            <div className="text-base font-bold text-emerald-300">Triage Complete!</div>
            <p className="text-xs text-zinc-400">
              Successfully sorted {result.moved} items with paired sidecars. Ledger saved.
            </p>
            <button
              onClick={onClose}
              className="bg-emerald-600 hover:bg-emerald-500 text-white font-medium px-5 py-2 rounded-xl text-xs cursor-pointer"
            >
              Close & Finish
            </button>
          </div>
        ) : (
          <div className="flex items-center justify-end gap-3 pt-2">
            <button
              onClick={onClose}
              disabled={isExecuting}
              className="px-4 py-2.5 rounded-xl border border-zinc-800 text-zinc-400 hover:text-white text-xs cursor-pointer"
            >
              Cancel
            </button>
            <button
              onClick={handleRunExecution}
              disabled={isExecuting}
              className="flex items-center gap-2 bg-emerald-600 hover:bg-emerald-500 text-white font-semibold px-6 py-2.5 rounded-xl text-sm shadow-lg shadow-emerald-950 transition active:scale-95 cursor-pointer"
            >
              {isExecuting ? (
                <>
                  <RefreshCw className="w-4 h-4 animate-spin" />
                  <span>Processing...</span>
                </>
              ) : (
                <>
                  <Play className="w-4 h-4" />
                  <span>Execute Move Now</span>
                </>
              )}
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
