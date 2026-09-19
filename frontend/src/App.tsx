import { useState, useEffect } from 'react';
import {
  FolderOpen,
  Play,
  RefreshCw,
  Calendar,
  Filter,
  X,
  RotateCcw,
  CheckCircle2,
  Settings
} from 'lucide-react';
import {
  type MediaItem,
  type Cluster,
  type SystemStatus,
  type ScanResult,
  fetchSystemStatus,
  scanFolder,
  clusterMedia,
  rollbackTriage,
  type AppSettings,
  fetchSettings
} from './services/api';
import { CleanSweepView } from './components/CleanSweepView';
import { DecisionDeckView } from './components/DecisionDeckView';
import { ClusterView } from './components/ClusterView';
import { ExecutionModal } from './components/ExecutionModal';
import { SettingsView } from './components/SettingsView';
import { MediaLibraryView } from './components/MediaLibraryView';
import { DuplicateReviewView } from './components/DuplicateReviewView';

type Step = 'ingest' | 'clean_sweep' | 'decision_deck' | 'clustering';

export function App() {
  const [step, setStep] = useState<Step>('ingest');
  const [sourceDir, setSourceDir] = useState<string>('C:\\Transfer\\Takeout\\K Photos\\2024');
  const [settings, setSettings] = useState<AppSettings>({
    source_dir: 'C:\\Transfer\\Takeout\\K Photos\\2024',
    pictures_dir: null,
    videos_dir: null,
  });
  const [showSettings, setShowSettings] = useState(false);
  const [showLibrary, setShowLibrary] = useState(false);
  const [showDuplicates, setShowDuplicates] = useState(false);
  const [monthFilter, setMonthFilter] = useState<string>('');
  const [scanStats, setScanStats] = useState<ScanResult | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [isRollingBack, setIsRollingBack] = useState(false);
  const [rollbackStatus, setRollbackStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // System & Model Status
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [backend, setBackend] = useState<'ollama' | 'gemini'>('ollama');

  // Scanned Categories
  const [obviousDocs, setObviousDocs] = useState<MediaItem[]>([]);
  const [obviousPhotos, setObviousPhotos] = useState<MediaItem[]>([]);
  const [mixedItems, setMixedItems] = useState<MediaItem[]>([]);

  // Triage Decisions
  const [approvedDocs, setApprovedDocs] = useState<MediaItem[]>([]);
  const [approvedPhotos, setApprovedPhotos] = useState<MediaItem[]>([]);
  const [approvedTrash, setApprovedTrash] = useState<MediaItem[]>([]);
  const [approvedSkipped, setApprovedSkipped] = useState<MediaItem[]>([]);
  const [clusters, setClusters] = useState<Cluster[]>([]);
  const [showExecutionModal, setShowExecutionModal] = useState(false);

  const checkStatus = () => {
    fetchSystemStatus()
      .then(setStatus)
      .catch(() => setStatus(null));
  };

  useEffect(() => {
    fetchSettings()
      .then((loaded) => {
        setSettings(loaded);
        setSourceDir(loaded.source_dir);
      })
      .catch(() => undefined);
    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  const handleStartScan = async () => {
    setIsScanning(true);
    setError(null);
    setRollbackStatus(null);
    setApprovedDocs([]);
    setApprovedPhotos([]);
    setApprovedTrash([]);
    setApprovedSkipped([]);
    setClusters([]);
    try {
      const res = await scanFolder(
        sourceDir,
        backend,
        'moondream',
        monthFilter.trim() || undefined,
        false
      );
      setScanStats(res);
      setObviousDocs(res.obvious_docs);
      setObviousPhotos(res.obvious_photos);
      setMixedItems(res.mixed_items);

      if (res.obvious_docs.length > 0 || res.obvious_photos.length > 0) {
        setStep('clean_sweep');
      } else if (res.mixed_items.length > 0) {
        setStep('decision_deck');
      } else {
        setError('No unprocessed media items found matching this filter.');
      }
    } catch (err: any) {
      setError(err.message || 'Scan failed');
    } finally {
      setIsScanning(false);
    }
  };

  const handleRollback = async () => {
    if (!sourceDir || isRollingBack) return;
    if (
      !window.confirm(
        `Roll back previous triage execution in:\n${sourceDir}?\n\nThis will restore moved files and sidecars to their pre-triage locations.`
      )
    ) {
      return;
    }

    setIsRollingBack(true);
    setError(null);
    setRollbackStatus(null);
    try {
      const res = await rollbackTriage(sourceDir);
      setRollbackStatus(
        `Successfully restored ${res.restored_items} media items and ${res.restored_sidecars} sidecars to pre-triage locations.`
      );
      setScanStats(null);
      setObviousDocs([]);
      setObviousPhotos([]);
      setMixedItems([]);
      setApprovedDocs([]);
      setApprovedPhotos([]);
      setApprovedTrash([]);
      setApprovedSkipped([]);
      setClusters([]);
    } catch (err: any) {
      setError(err.message || 'Rollback failed');
    } finally {
      setIsRollingBack(false);
    }
  };

  const handleCleanSweepApprove = (
    validDocs: MediaItem[],
    validPhotos: MediaItem[],
    demotedToMixed: MediaItem[],
    trashItems: MediaItem[] = []
  ) => {
    setApprovedDocs(validDocs);
    setApprovedPhotos(validPhotos);
    setApprovedTrash((prev) => [...prev, ...trashItems]);
    const combinedMixed = [...mixedItems, ...demotedToMixed];
    setMixedItems(combinedMixed);

    if (combinedMixed.length > 0) {
      setStep('decision_deck');
    } else {
      handleProceedToClustering(validPhotos);
    }
  };

  const handleDecisionDeckComplete = async (
    deckDecisions: Record<string, { item: MediaItem; decision: 'DOCUMENT' | 'PHOTO' | 'TRASH' | 'SKIP' }>
  ) => {
    const deckDocs: MediaItem[] = [];
    const deckPhotos: MediaItem[] = [];
    const deckTrash: MediaItem[] = [];
    const deckSkipped: MediaItem[] = [];

    Object.values(deckDecisions).forEach(({ item, decision }) => {
      if (decision === 'DOCUMENT') deckDocs.push(item);
      else if (decision === 'PHOTO') deckPhotos.push(item);
      else if (decision === 'TRASH') deckTrash.push(item);
      else if (decision === 'SKIP') deckSkipped.push(item);
    });

    const allDocs = [...approvedDocs, ...deckDocs];
    const allPhotos = [...approvedPhotos, ...deckPhotos];
    const allTrash = [...approvedTrash, ...deckTrash];
    const allSkipped = [...approvedSkipped, ...deckSkipped];

    setApprovedDocs(allDocs);
    setApprovedPhotos(allPhotos);
    setApprovedTrash(allTrash);
    setApprovedSkipped(allSkipped);

    handleProceedToClustering(allPhotos);
  };

  const handleProceedToClustering = async (photos: MediaItem[]) => {
    try {
      const res = await clusterMedia(photos);
      setClusters(res.clusters);
      setStep('clustering');
    } catch (err: any) {
      setError(err.message || 'Clustering failed');
    }
  };

  const handleClusterCommit = (customNames: Record<string, string>) => {
    setClusters((prev) =>
      prev.map((c) => ({
        ...c,
        folder_name: customNames[c.id] || c.folder_name,
      }))
    );
    setShowExecutionModal(true);
  };

  // Compile full decisions map for executor
  const buildFinalDecisions = () => {
    const decisions: Record<string, any> = {};

    // 1. Documents
    approvedDocs.forEach((d) => {
      decisions[d.id] = {
        path: d.path,
        category: 'DOCUMENT',
        is_video: d.is_video,
        month_str: d.month_str,
        has_sidecar: d.has_sidecar,
        sidecar_path: d.sidecar_path,
      };
    });

    // 2. Clustered Photos & Videos
    clusters.forEach((c) => {
      c.items.forEach((it) => {
        decisions[it.id] = {
          path: it.path,
          category: 'PHOTO',
          is_video: it.is_video,
          folder_name: c.folder_name,
          month_str: it.month_str,
          has_sidecar: it.has_sidecar,
          sidecar_path: it.sidecar_path,
        };
      });
    });

    // 3. Utility Trash
    approvedTrash.forEach((t) => {
      decisions[t.id] = {
        path: t.path,
        category: 'TRASH',
        is_video: t.is_video,
        month_str: t.month_str,
        has_sidecar: t.has_sidecar,
        sidecar_path: t.sidecar_path,
      };
    });

    // 4. Skipped Items
    approvedSkipped.forEach((s) => {
      decisions[s.id] = {
        path: s.path,
        category: 'SKIP',
        is_video: s.is_video,
        month_str: s.month_str,
        has_sidecar: s.has_sidecar,
        sidecar_path: s.sidecar_path,
      };
    });

    return decisions;
  };

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 flex flex-col font-sans select-none">
      {/* Top Cockpit Navbar */}
      <header className="h-16 border-b border-zinc-800 bg-zinc-900/60 backdrop-blur-xl px-6 flex items-center justify-between sticky top-0 z-40">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-purple-600 to-cyan-500 flex items-center justify-center text-lg shadow-lg shadow-cyan-950">
            🕸️
          </div>
          <div>
            <h1 className="text-base font-bold text-white tracking-wide flex items-center gap-2">
              DreamCatcher
              <span className="text-[10px] uppercase font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">
                Photo Triage
              </span>
            </h1>
          </div>
        </div>

        {/* Step Indicator */}
        <div className="hidden md:flex items-center gap-2 text-xs">
          <span
            className={`px-3 py-1 rounded-lg border transition ${
              step === 'ingest'
                ? 'bg-cyan-500/20 text-cyan-300 border-cyan-500/40 font-bold'
                : 'text-zinc-500 border-transparent'
            }`}
          >
            1. Ingest
          </span>
          <span className="text-zinc-600">→</span>
          <span
            className={`px-3 py-1 rounded-lg border transition ${
              step === 'clean_sweep'
                ? 'bg-emerald-500/20 text-emerald-300 border-emerald-500/40 font-bold'
                : 'text-zinc-500 border-transparent'
            }`}
          >
            2. Clean Sweep
          </span>
          <span className="text-zinc-600">→</span>
          <span
            className={`px-3 py-1 rounded-lg border transition ${
              step === 'decision_deck'
                ? 'bg-amber-500/20 text-amber-300 border-amber-500/40 font-bold'
                : 'text-zinc-500 border-transparent'
            }`}
          >
            3. Decision Deck
          </span>
          <span className="text-zinc-600">→</span>
          <span
            className={`px-3 py-1 rounded-lg border transition ${
              step === 'clustering'
                ? 'bg-purple-500/20 text-purple-300 border-purple-500/40 font-bold'
                : 'text-zinc-500 border-transparent'
            }`}
          >
            4. Event Studio
          </span>
        </div>

        {/* System & Ollama Status */}
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={checkStatus}
            title="Click to refresh Ollama connection status"
            className="flex items-center gap-2 bg-zinc-950 hover:bg-zinc-900 px-3 py-1.5 rounded-xl border border-zinc-800 hover:border-zinc-700 text-xs transition cursor-pointer"
          >
            <span
              className={`w-2 h-2 rounded-full ${
                status?.ollama.connected ? 'bg-emerald-400 animate-pulse' : 'bg-rose-500'
              }`}
            />
            <span className="text-zinc-300 font-medium">
              {status?.ollama.connected
                ? status.ollama.has_moondream
                  ? 'Moondream Ready'
                  : 'Ollama Online'
                : 'Ollama Offline (Retry)'}
            </span>
          </button>

          <select
            value={backend}
            onChange={(e) => setBackend(e.target.value as any)}
            className="bg-zinc-900 text-xs border border-zinc-700 rounded-xl px-2.5 py-1.5 text-zinc-300 focus:outline-none cursor-pointer"
          >
            <option value="ollama">Local Moondream</option>
            <option value="gemini">Gemini Flash-Lite</option>
          </select>

          <button
            type="button"
            onClick={() => setShowSettings(true)}
            className="flex items-center gap-2 bg-zinc-950 hover:bg-zinc-900 px-3 py-1.5 rounded-xl border border-zinc-800 hover:border-zinc-700 text-xs transition cursor-pointer"
            title="Open folder settings"
          >
            <Settings className="w-3.5 h-3.5 text-zinc-400" />
            <span className="text-zinc-300 font-medium">Settings</span>
          </button>
          <button
            type="button"
            onClick={() => setShowLibrary(true)}
            className="flex items-center gap-2 bg-zinc-950 hover:bg-zinc-900 px-3 py-1.5 rounded-xl border border-zinc-800 hover:border-zinc-700 text-xs transition cursor-pointer"
            title="Open media library"
          >
            <FolderOpen className="w-3.5 h-3.5 text-zinc-400" />
            <span className="text-zinc-300 font-medium">Library</span>
          </button>
          <button
            type="button"
            onClick={() => setShowDuplicates(true)}
            className="flex items-center gap-2 bg-zinc-950 hover:bg-zinc-900 px-3 py-1.5 rounded-xl border border-zinc-800 hover:border-zinc-700 text-xs transition cursor-pointer"
            title="Review duplicate candidates"
          >
            <span className="text-zinc-300 font-medium">Duplicates</span>
          </button>
        </div>
      </header>

      {/* Main View Container */}
      <main className="flex-1 p-6 max-w-7xl w-full mx-auto flex flex-col">
        {showSettings ? (
          <SettingsView
            settings={settings}
            onSaved={(saved) => {
              setSettings(saved);
              setSourceDir(saved.source_dir);
            }}
            onClose={() => setShowSettings(false)}
          />
        ) : showLibrary ? (
          <MediaLibraryView onClose={() => setShowLibrary(false)} />
        ) : showDuplicates ? (
          <DuplicateReviewView onClose={() => setShowDuplicates(false)} />
        ) : (
          <>
        {rollbackStatus && (
          <div className="mb-4 p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-300 text-sm flex items-center justify-between">
            <div className="flex items-center gap-2">
              <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
              <span>{rollbackStatus}</span>
            </div>
            <button onClick={() => setRollbackStatus(null)} className="text-xs hover:underline cursor-pointer">
              Dismiss
            </button>
          </div>
        )}

        {error && (
          <div className="mb-4 p-4 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-sm flex items-center justify-between">
            <span>{error}</span>
            <button onClick={() => setError(null)} className="text-xs hover:underline cursor-pointer">
              Dismiss
            </button>
          </div>
        )}

        {/* STEP 1: Ingest & Scanner */}
        {step === 'ingest' && (
          <div className="flex flex-col items-center justify-center my-auto space-y-8 max-w-xl mx-auto w-full text-center">
            <div className="w-20 h-20 rounded-3xl bg-gradient-to-tr from-purple-600/30 to-cyan-500/30 border border-cyan-500/30 flex items-center justify-center text-4xl shadow-2xl shadow-cyan-950">
              📸
            </div>

            <div className="space-y-2">
              <h2 className="text-3xl font-extrabold text-white tracking-tight">
                Welcome to DreamCatcher
              </h2>
              <p className="text-sm text-zinc-400 max-w-md mx-auto">
                Select your unzipped Google Photos Takeout folder to begin AI-assisted triage with Moondream.
              </p>
            </div>

            <div className="w-full space-y-4 bg-zinc-900/60 p-6 rounded-3xl border border-zinc-800/80 backdrop-blur-md">
              <div className="space-y-1.5 text-left">
                <label className="text-xs font-semibold text-zinc-400">Target Working Folder</label>
                <div className="flex items-center gap-2 bg-zinc-950 border border-zinc-800 rounded-xl p-2 focus-within:border-cyan-500 transition">
                  <FolderOpen className="w-5 h-5 text-zinc-500 shrink-0 ml-1" />
                  <input
                    type="text"
                    value={sourceDir}
                    onChange={(e) => setSourceDir(e.target.value)}
                    placeholder="e.g. C:\Transfer\Takeout\K Photos\2024"
                    className="bg-transparent w-full text-sm text-white focus:outline-none"
                  />
                </div>
              </div>

              {/* Quick Presets */}
              <div className="flex items-center gap-2 text-xs text-zinc-400">
                <span>Quick:</span>
                <button
                  onClick={() => setSourceDir('C:\\Transfer\\Takeout\\K Photos\\2024')}
                  className="px-2 py-0.5 rounded-md bg-zinc-800 hover:bg-zinc-700 text-zinc-300 cursor-pointer"
                >
                  2024 Photos
                </button>
                <button
                  onClick={() => setSourceDir('C:\\Transfer\\Takeout')}
                  className="px-2 py-0.5 rounded-md bg-zinc-800 hover:bg-zinc-700 text-zinc-300 cursor-pointer"
                >
                  Takeout Root
                </button>
              </div>

              {/* Month Filter Section */}
              <div className="space-y-1.5 text-left border-t border-zinc-800/80 pt-3">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-semibold text-zinc-400 flex items-center gap-1.5">
                    <Calendar className="w-3.5 h-3.5 text-cyan-400" />
                    <span>Month Filter (Optional YYYY-MM)</span>
                  </label>
                  {monthFilter && (
                    <button
                      onClick={() => setMonthFilter('')}
                      className="text-[11px] text-zinc-500 hover:text-zinc-300 flex items-center gap-1 cursor-pointer"
                    >
                      <X className="w-3 h-3" />
                      Clear Filter
                    </button>
                  )}
                </div>
                <div className="flex items-center gap-2 bg-zinc-950 border border-zinc-800 rounded-xl p-2 focus-within:border-cyan-500 transition">
                  <Filter className="w-4 h-4 text-zinc-500 shrink-0 ml-1" />
                  <input
                    type="text"
                    value={monthFilter}
                    onChange={(e) => setMonthFilter(e.target.value)}
                    placeholder="e.g. 2024-11 (Leave empty for all months)"
                    className="bg-transparent w-full text-sm text-white focus:outline-none"
                  />
                </div>
                <div className="flex items-center gap-2 text-[11px] text-zinc-400 pt-0.5">
                  <span>Presets:</span>
                  <button
                    onClick={() => setMonthFilter('')}
                    className={`px-2 py-0.5 rounded transition cursor-pointer ${
                      !monthFilter ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/30' : 'bg-zinc-800 hover:bg-zinc-700 text-zinc-400'
                    }`}
                  >
                    All Months
                  </button>
                  <button
                    onClick={() => setMonthFilter('2024-11')}
                    className={`px-2 py-0.5 rounded transition cursor-pointer ${
                      monthFilter === '2024-11' ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/30' : 'bg-zinc-800 hover:bg-zinc-700 text-zinc-400'
                    }`}
                  >
                    2024-11
                  </button>
                  <button
                    onClick={() => setMonthFilter('2024-12')}
                    className={`px-2 py-0.5 rounded transition cursor-pointer ${
                      monthFilter === '2024-12' ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/30' : 'bg-zinc-800 hover:bg-zinc-700 text-zinc-400'
                    }`}
                  >
                    2024-12
                  </button>
                </div>
              </div>

              {/* Scanned Count Indicator */}
              {scanStats && (
                <div className="p-3 bg-zinc-950/60 rounded-xl border border-zinc-800 text-xs text-zinc-300 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-emerald-400"></span>
                    <span>
                      Scanned: <strong>{scanStats.total_scanned}</strong> items
                      {scanStats.month_filter ? ` for ${scanStats.month_filter}` : ' across all months'}
                    </span>
                  </div>
                  {scanStats.total_discovered > scanStats.total_scanned && (
                    <span className="text-zinc-500 text-[11px]">
                      ({scanStats.total_discovered} total files in folder)
                    </span>
                  )}
                </div>
              )}

              <button
                onClick={handleStartScan}
                disabled={isScanning || isRollingBack || !sourceDir}
                className="w-full flex items-center justify-center gap-2 bg-cyan-600 hover:bg-cyan-500 text-white font-semibold py-3.5 rounded-xl shadow-lg shadow-cyan-950 transition active:scale-98 cursor-pointer disabled:opacity-50"
              >
                {isScanning ? (
                  <>
                    <RefreshCw className="w-4 h-4 animate-spin" />
                    <span>Scanning & Triaging Media...</span>
                  </>
                ) : (
                  <>
                    <Play className="w-4 h-4" />
                    <span>
                      {monthFilter ? `Start Triage for ${monthFilter}` : 'Start Photo Triage'}
                    </span>
                  </>
                )}
              </button>

              {/* Rollback Action */}
              <div className="pt-2 border-t border-zinc-800/80 flex items-center justify-center">
                <button
                  type="button"
                  onClick={handleRollback}
                  disabled={isScanning || isRollingBack || !sourceDir}
                  className="flex items-center gap-1.5 text-xs text-zinc-400 hover:text-amber-300 transition py-1.5 px-3 rounded-lg hover:bg-zinc-800/60 cursor-pointer disabled:opacity-50"
                  title="Restore moved files and sidecars to their pre-triage state"
                >
                  <RotateCcw className={`w-3.5 h-3.5 ${isRollingBack ? 'animate-spin text-amber-400' : ''}`} />
                  <span>{isRollingBack ? 'Rolling back files...' : 'Rollback Previous Run'}</span>
                </button>
              </div>
            </div>
          </div>
        )}

        {/* STEP 2: Clean Sweep */}
        {step === 'clean_sweep' && (
          <CleanSweepView
            obviousDocs={obviousDocs}
            obviousPhotos={obviousPhotos}
            onApprove={handleCleanSweepApprove}
          />
        )}

        {/* STEP 3: Decision Deck */}
        {step === 'decision_deck' && (
          <DecisionDeckView
            mixedItems={mixedItems}
            backend={backend}
            onComplete={handleDecisionDeckComplete}
            onBack={() => setStep('clean_sweep')}
          />
        )}

        {/* STEP 4: Event Clustering Studio */}
        {step === 'clustering' && (
          <ClusterView
            clusters={clusters}
            onCommit={handleClusterCommit}
            onBack={() => setStep('decision_deck')}
          />
        )}
          </>
        )}
      </main>

      {/* Execution Modal */}
      {showExecutionModal && (
        <ExecutionModal
          sourceDir={sourceDir}
          picturesDir={settings.pictures_dir}
          videosDir={settings.videos_dir}
          decisions={buildFinalDecisions()}
          onClose={() => setShowExecutionModal(false)}
          onSuccess={() => {
            setApprovedDocs([]);
            setApprovedPhotos([]);
            setApprovedTrash([]);
            setApprovedSkipped([]);
            setClusters([]);
            setStep('ingest');
          }}
        />
      )}
    </div>
  );
}

export default App;
