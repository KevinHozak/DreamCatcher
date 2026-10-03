import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Check, RefreshCw, ShieldCheck, X } from 'lucide-react';
import { analyzeDuplicates, excludeDuplicateMember, exportDuplicateProposal, fetchDuplicateGroups, isTauri, type DuplicateGroup } from '../services/api';

interface Props { onClose: () => void; }

function bytes(value: number) { return value < 1024 ? `${value} B` : `${(value / 1024 / 1024).toFixed(1)} MB`; }

export function DuplicateReviewView({ onClose }: Props) {
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [memberPages, setMemberPages] = useState<Record<string, number>>({});
  const [pendingMember, setPendingMember] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [progress, setProgress] = useState<{ processed: number; total: number } | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = (targetPage = page) => {
    return fetchDuplicateGroups(targetPage).then((result) => { setGroups(result.groups); setTotal(result.total); }).catch((err) => setError(err instanceof Error ? err.message : 'Could not load duplicates')).finally(() => setLoading(false));
  };

  useEffect(() => {
    let cancelled = false;
    fetchDuplicateGroups(page).then((result) => { if (!cancelled) { setGroups(result.groups); setTotal(result.total); } }).catch((err) => { if (!cancelled) setError(err instanceof Error ? err.message : 'Could not load duplicates'); }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [page]);

  useEffect(() => {
    if (!isTauri) return;
    let disposed = false;
    let cleanup: (() => void) | undefined;
    listen<{ processed: number; total: number }>('duplicate-analysis-progress', (event) => setProgress(event.payload)).then((unlisten) => { if (disposed) unlisten(); else cleanup = unlisten; }).catch(() => undefined);
    return () => { disposed = true; cleanup?.(); };
  }, []);

  const analyze = async () => {
    setAnalyzing(true);
    setProgress(null);
    setMemberPages({});
    setError(null);
    try { await analyzeDuplicates(); setPage(1); await load(1); } catch (err) { setError(err instanceof Error ? err.message : 'Duplicate analysis failed'); } finally { setAnalyzing(false); }
  };

  const toggleExcluded = async (group: DuplicateGroup, identity: string, excluded: boolean) => {
    setPendingMember(identity);
    try {
      await excludeDuplicateMember(group.group_id, identity, excluded);
      setGroups((current) => current.map((item) => item.group_id === group.group_id ? { ...item, members: item.members.map((member) => member.identity === identity ? { ...member, excluded } : member) } : item));
    } catch (err) { setError(err instanceof Error ? err.message : 'Could not update exclusion'); } finally { setPendingMember(null); }
  };

  const exportProposal = async () => {
    setExporting(true);
    try { await exportDuplicateProposal(); } catch (err) { setError(err instanceof Error ? err.message : 'Could not export proposal'); } finally { setExporting(false); }
  };

  return <section className="max-w-5xl mx-auto w-full space-y-5">
    <div className="flex items-start justify-between"><div><p className="text-xs uppercase tracking-widest text-amber-400 font-semibold">Review only</p><h2 className="text-3xl font-extrabold text-white mt-1">Duplicate Candidates</h2><p className="text-sm text-zinc-400 mt-2">Exact matches are grouped for review. Nothing is deleted automatically.</p></div><button type="button" onClick={onClose}><X className="w-5 h-5 text-zinc-400" /></button></div>
    <div className="flex items-center justify-between bg-zinc-900/60 border border-zinc-800 rounded-2xl p-4"><div className="flex items-center gap-3 text-sm text-zinc-300"><ShieldCheck className="w-5 h-5 text-emerald-400" />Exact-content matching with reviewable exclusions</div><button type="button" onClick={analyze} disabled={analyzing || exporting || pendingMember !== null} className="flex items-center gap-2 px-4 py-2 rounded-xl bg-amber-600 hover:bg-amber-500 disabled:opacity-50 text-sm font-semibold"><RefreshCw className={`w-4 h-4 ${analyzing ? 'animate-spin' : ''}`} />{analyzing ? (progress ? `Analyzing ${progress.processed.toLocaleString()} / ${progress.total.toLocaleString()}…` : 'Analyzing…') : 'Analyze library'}</button></div>
    <div className="flex items-center gap-3 text-sm text-zinc-300"><button type="button" onClick={exportProposal} disabled={analyzing || exporting} className="px-3 py-2 rounded-xl border border-zinc-700 disabled:opacity-50">{exporting ? 'Exporting…' : 'Export review proposal'}</button><button type="button" disabled={page <= 1 || analyzing} onClick={() => setPage(page - 1)}>Previous</button><span>Page {page} of {Math.max(1, Math.ceil(total / 50))} · {total} groups</span><button type="button" disabled={page * 50 >= total || analyzing} onClick={() => setPage(page + 1)}>Next</button></div>
    {error && <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-sm text-rose-300">{error}</div>}
    {loading ? <div className="p-12 text-center text-sm text-zinc-500">Loading duplicate groups…</div> : groups.length === 0 ? <div className="p-12 text-center border border-dashed border-zinc-800 rounded-2xl text-sm text-zinc-500">No duplicate groups found. Run an analysis to check the indexed library.</div> : <div className="space-y-4">{groups.map((group) => <div key={group.group_id} className="bg-zinc-900/60 border border-zinc-800 rounded-2xl p-4 space-y-3"><div className="flex justify-between text-sm"><span className="font-semibold text-zinc-200">Exact match · {group.members.length} files</span><span className="text-zinc-500">{group.fingerprint.slice(0, 12)}…</span></div>{group.members.slice((memberPages[group.group_id] || 0) * 50, ((memberPages[group.group_id] || 0) + 1) * 50).map((member) => <div key={member.identity} className={`flex items-center gap-3 p-3 rounded-xl border ${member.excluded ? 'border-zinc-800 opacity-50' : 'border-zinc-700'}`}><span className="flex-1 min-w-0"><span className="block truncate text-sm text-zinc-200">{member.path}</span><span className="text-xs text-zinc-500">{bytes(member.size)} · {new Date(member.timestamp).toLocaleString()}</span></span><button type="button" disabled={analyzing || pendingMember !== null} onClick={() => toggleExcluded(group, member.identity, !member.excluded)} className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg border border-zinc-700 text-xs text-zinc-300 hover:text-white">{member.excluded ? <><Check className="w-3 h-3" />Excluded</> : 'Exclude'}</button></div>)}{group.members.length > 50 && <div className="flex gap-3 text-xs text-zinc-300"><button type="button" disabled={!memberPages[group.group_id]} onClick={() => setMemberPages((current) => ({ ...current, [group.group_id]: (current[group.group_id] || 0) - 1 }))}>Previous files</button><span>Files page {(memberPages[group.group_id] || 0) + 1} of {Math.ceil(group.members.length / 50)}</span><button type="button" disabled={((memberPages[group.group_id] || 0) + 1) * 50 >= group.members.length} onClick={() => setMemberPages((current) => ({ ...current, [group.group_id]: (current[group.group_id] || 0) + 1 }))}>Next files</button></div>}</div>)}</div>}
  </section>;
}
