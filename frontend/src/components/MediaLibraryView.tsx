import { useEffect, useState } from 'react';
import { CalendarDays, ChevronLeft, ChevronRight, FileImage, FolderOpen, Search, X } from 'lucide-react';
import { deletePeopleSearchIndex, fetchInventoryStats, fetchPeopleSearchStatus, processMediaCaptions, queryInventory, saveMediaDescription, type InventoryItem, type InventoryPage, type InventoryStats, type PeopleSearchStatus } from '../services/api';

interface Props {
  onClose: () => void;
}

const PAGE_SIZE = 50;

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 0; value >= 1024 && index < units.length - 1; index += 1) {
    value /= 1024;
    unit = units[index + 1];
  }
  return `${value.toFixed(value >= 100 ? 0 : 1)} ${unit}`;
}

export function MediaLibraryView({ onClose }: Props) {
  const [rootKind, setRootKind] = useState<'pictures' | 'videos'>('pictures');
  const [search, setSearch] = useState('');
  const [year, setYear] = useState('');
  const [extension, setExtension] = useState('');
  const [state, setState] = useState<'available' | 'stale'>('available');
  const [page, setPage] = useState<InventoryPage | null>(null);
  const [stats, setStats] = useState<InventoryStats | null>(null);
  const [peopleSearch, setPeopleSearch] = useState<PeopleSearchStatus | null>(null);
  const [selected, setSelected] = useState<InventoryItem | null>(null);
  const [pageNumber, setPageNumber] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [captionDraft, setCaptionDraft] = useState('');
  const [processingCaptions, setProcessingCaptions] = useState(false);

  useEffect(() => {
    fetchInventoryStats().then(setStats).catch(() => undefined);
    fetchPeopleSearchStatus().then(setPeopleSearch).catch(() => undefined);
  }, []);

  const handleDeletePeopleIndex = async () => {
    if (!peopleSearch?.index_exists || !window.confirm('Delete the derived people-search index? Media files and the canonical inventory will not be changed.')) return;
    await deletePeopleSearchIndex();
    setPeopleSearch((current) => current ? { ...current, state: current.enabled ? 'not_ready' : 'disabled', index_exists: false, indexed_media: 0, reviewed_people: 0, last_run: null } : current);
  };

  useEffect(() => {
    queryInventory({
      rootKind,
      search,
      year: year ? Number(year) : undefined,
      extension,
      state,
      page: pageNumber,
      pageSize: PAGE_SIZE,
    })
      .then(setPage)
      .catch((err) => setError(err instanceof Error ? err.message : 'Could not load media library'))
      .finally(() => setLoading(false));
  }, [rootKind, search, year, extension, state, pageNumber]);

  const totalPages = page ? Math.max(1, Math.ceil(page.total / PAGE_SIZE)) : 1;

  const clearFilters = () => {
    setSearch('');
    setYear('');
    setExtension('');
    setState('available');
    setPageNumber(1);
  };

  const handleProcessCaptions = async () => {
    setProcessingCaptions(true);
    try {
      await processMediaCaptions(rootKind);
      setPageNumber((value) => value);
      const refreshed = await queryInventory({ rootKind, search, year: year ? Number(year) : undefined, extension, state, page: pageNumber, pageSize: PAGE_SIZE });
      setPage(refreshed);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Caption processing is unavailable');
    } finally {
      setProcessingCaptions(false);
    }
  };

  const handleSaveCaption = async () => {
    if (!selected) return;
    try {
      await saveMediaDescription(selected.identity, captionDraft.trim() || null);
      setSelected({ ...selected, description: captionDraft.trim() || null, description_source: captionDraft.trim() ? 'user' : 'embedded', description_status: captionDraft.trim() ? 'complete' : 'not_processed' });
      setPage((current) => current ? { ...current, items: current.items.map((item) => item.identity === selected.identity ? { ...item, description: captionDraft.trim() || null, description_source: captionDraft.trim() ? 'user' : 'embedded', description_status: captionDraft.trim() ? 'complete' : 'not_processed' } : item) } : current);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not save description');
    }
  };

  return (
    <section className="max-w-7xl mx-auto w-full space-y-5">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-xs uppercase tracking-widest text-cyan-400 font-semibold">Inventory</p>
          <h2 className="text-3xl font-extrabold text-white mt-1">Media Library</h2>
          <p className="text-sm text-zinc-400 mt-2">Browse indexed media without changing files on disk.</p>
        </div>
        <button type="button" onClick={onClose} className="p-2 rounded-xl text-zinc-400 hover:text-white hover:bg-zinc-800 transition">
          <X className="w-5 h-5" />
        </button>
      </div>

      {stats && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <div className="bg-zinc-900/70 border border-zinc-800 rounded-2xl p-4"><p className="text-xs text-zinc-500">Pictures</p><p className="text-xl font-bold text-cyan-400">{stats.pictures_count.toLocaleString()}</p><p className="text-xs text-zinc-500">{formatBytes(stats.pictures_bytes)}</p></div>
          <div className="bg-zinc-900/70 border border-zinc-800 rounded-2xl p-4"><p className="text-xs text-zinc-500">Videos</p><p className="text-xl font-bold text-purple-400">{stats.videos_count.toLocaleString()}</p><p className="text-xs text-zinc-500">{formatBytes(stats.videos_bytes)}</p></div>
          <div className="bg-zinc-900/70 border border-zinc-800 rounded-2xl p-4"><p className="text-xs text-zinc-500">Stale records</p><p className="text-xl font-bold text-amber-400">{stats.stale_count.toLocaleString()}</p><p className="text-xs text-zinc-500">Awaiting a successful rescan</p></div>
          <div className="bg-zinc-900/70 border border-zinc-800 rounded-2xl p-4"><p className="text-xs text-zinc-500">Last scan</p><p className="text-xl font-bold text-emerald-400">{stats.last_scan?.status || 'Not scanned'}</p><p className="text-xs text-zinc-500 truncate">{stats.last_scan?.completed_at || 'No scan recorded'}</p></div>
        </div>
      )}

      {peopleSearch && (
        <div className="flex flex-wrap items-center gap-3 bg-zinc-900/60 border border-zinc-800 rounded-2xl p-4">
          <div className="flex-1 min-w-60">
            <p className="text-xs uppercase tracking-widest text-purple-400 font-semibold">People search</p>
            <p className="text-sm text-zinc-200 mt-1">{peopleSearch.message}</p>
            <p className="text-xs text-zinc-500 mt-1">State: {peopleSearch.state} · {peopleSearch.reviewed_people} reviewed people · {peopleSearch.indexed_media} indexed media</p>
          </div>
          {peopleSearch.index_exists && <button type="button" onClick={handleDeletePeopleIndex} className="px-3 py-2 rounded-xl border border-rose-500/30 text-xs text-rose-300 hover:bg-rose-500/10 transition">Delete derived index</button>}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2">
        {(['pictures', 'videos'] as const).map((kind) => (
          <button key={kind} type="button" onClick={() => { setRootKind(kind); setPageNumber(1); setSelected(null); }} className={`px-4 py-2 rounded-xl text-sm font-semibold border transition ${rootKind === kind ? 'bg-cyan-500/20 text-cyan-300 border-cyan-500/40' : 'bg-zinc-900 text-zinc-400 border-zinc-800 hover:text-white'}`}>
            {kind === 'pictures' ? 'Pictures' : 'Videos'}
          </button>
        ))}
        <div className="flex-1" />
        <button type="button" onClick={handleProcessCaptions} disabled={processingCaptions} className="px-3 py-2 rounded-xl border border-purple-500/30 text-xs text-purple-300 hover:bg-purple-500/10 disabled:opacity-50 transition">{processingCaptions ? 'Processing…' : 'Process captions'}</button>
        <button type="button" onClick={clearFilters} className="text-xs text-zinc-500 hover:text-zinc-200 transition">Clear filters</button>
      </div>

      <div className="flex flex-wrap gap-2 bg-zinc-900/60 border border-zinc-800 rounded-2xl p-3">
        <div className="flex items-center gap-2 flex-1 min-w-56 bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2">
          <Search className="w-4 h-4 text-zinc-500" />
          <input value={search} onChange={(event) => { setSearch(event.target.value); setPageNumber(1); }} placeholder="Search filename or path" className="bg-transparent text-sm text-white outline-none w-full" />
        </div>
        <div className="flex items-center gap-2 bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2"><CalendarDays className="w-4 h-4 text-zinc-500" /><input value={year} onChange={(event) => { setYear(event.target.value.replace(/\D/g, '').slice(0, 4)); setPageNumber(1); }} placeholder="Year" inputMode="numeric" className="bg-transparent text-sm text-white outline-none w-20" /></div>
        <input value={extension} onChange={(event) => { setExtension(event.target.value); setPageNumber(1); }} placeholder="Extension" className="bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2 text-sm text-white outline-none w-28" />
        <select value={state} onChange={(event) => { setState(event.target.value as 'available' | 'stale'); setPageNumber(1); }} className="bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2 text-sm text-zinc-300 outline-none"><option value="available">Available</option><option value="stale">Stale</option></select>
      </div>

      {error && <div className="p-4 rounded-2xl bg-rose-500/10 border border-rose-500/30 text-sm text-rose-300">{error}</div>}
      {loading ? <div className="p-12 text-center text-sm text-zinc-500">Loading indexed media…</div> : page?.items.length === 0 ? <div className="p-12 text-center bg-zinc-900/40 border border-dashed border-zinc-800 rounded-2xl text-sm text-zinc-500">No indexed {rootKind} match these filters.</div> : (
        <div className="bg-zinc-900/50 border border-zinc-800 rounded-2xl overflow-hidden">
          <div className="divide-y divide-zinc-800/80">
            {page?.items.map((item) => (
              <button key={item.identity} type="button" onClick={() => { setSelected(item); setCaptionDraft(item.description || ''); }} className="w-full text-left flex items-center gap-3 px-4 py-3 hover:bg-zinc-800/50 transition">
                <FileImage className="w-5 h-5 text-cyan-400 shrink-0" />
                <span className="min-w-0 flex-1"><span className="block text-sm text-zinc-200 truncate">{item.path}</span><span className="block text-xs text-zinc-500">{item.extension} · {formatBytes(item.size)} · {new Date(item.timestamp).toLocaleDateString()}</span></span>
                {item.state === 'stale' && <span className="text-[10px] uppercase text-amber-400 border border-amber-500/30 rounded px-1.5 py-0.5">stale</span>}
              </button>
            ))}
          </div>
          <div className="flex items-center justify-between px-4 py-3 border-t border-zinc-800 text-xs text-zinc-500"><span>{page?.total.toLocaleString()} results · page {pageNumber} of {totalPages}</span><div className="flex gap-2"><button type="button" disabled={pageNumber <= 1} onClick={() => setPageNumber((value) => value - 1)} className="p-1.5 rounded-lg border border-zinc-800 disabled:opacity-30"><ChevronLeft className="w-4 h-4" /></button><button type="button" disabled={pageNumber >= totalPages} onClick={() => setPageNumber((value) => value + 1)} className="p-1.5 rounded-lg border border-zinc-800 disabled:opacity-30"><ChevronRight className="w-4 h-4" /></button></div></div>
        </div>
      )}

      {selected && <div className="fixed inset-0 z-50 bg-black/70 flex items-center justify-center p-4" onClick={() => setSelected(null)}><div className="bg-zinc-900 border border-zinc-700 rounded-2xl p-5 max-w-xl w-full space-y-4" onClick={(event) => event.stopPropagation()}><div className="flex items-center justify-between"><h3 className="font-bold text-white">Media details</h3><button type="button" onClick={() => setSelected(null)}><X className="w-4 h-4 text-zinc-400" /></button></div><div className="space-y-2 text-sm"><p className="break-all text-zinc-200"><FolderOpen className="inline w-4 h-4 mr-2 text-cyan-400" />{selected.path}</p><p className="text-zinc-400">{selected.media_type} · {formatBytes(selected.size)} · {selected.extension}</p><p className="text-zinc-400">Captured/indexed: {new Date(selected.timestamp).toLocaleString()}</p><p className="text-zinc-400">Sidecar: {selected.has_sidecar ? 'available' : 'not found'} · GPS: {selected.has_gps ? 'available' : 'not found'} · State: {selected.state}</p><label className="block text-xs uppercase tracking-widest text-zinc-500 pt-2">Description <span className="normal-case">({selected.description_source || 'not processed'})</span></label><textarea value={captionDraft} onChange={(event) => setCaptionDraft(event.target.value)} placeholder="Imported descriptions stay separate from the original file" className="w-full min-h-24 rounded-xl bg-zinc-950 border border-zinc-800 p-3 text-sm text-white outline-none" /><button type="button" onClick={handleSaveCaption} className="px-3 py-2 rounded-xl bg-cyan-500/20 border border-cyan-500/30 text-cyan-200 text-xs">Save description</button></div></div></div>}
    </section>
  );
}
