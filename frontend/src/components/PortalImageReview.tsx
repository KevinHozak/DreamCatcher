import { useEffect, useRef, useState } from 'react';
import { cancelPortalReview, finishPortalReview, preparePortalReview, sendPortalReview, type PortalPreview, type PortalSource, type PortalReceipt } from '../services/portalReview';

export function PortalImageReview({ initialSource = 'selection', onClose }: { initialSource?: PortalSource; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const current = useRef<PortalPreview | null>(null);
  const sequence = useRef(0);
  const sending = useRef<string | null>(null);
  const [receipt, setReceipt] = useState<PortalReceipt | null>(null);
  const [source, setSource] = useState<PortalSource>(initialSource);
  const [text, setText] = useState('Classify these generated colored shapes.');
  const [redact, setRedact] = useState(true);
  const [preview, setPreview] = useState<PortalPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [expired, setExpired] = useState(false);
  const active = useRef(true);
  const refresh = async (nextSource: PortalSource) => {
    const request = ++sequence.current;
    setBusy(true); setPreview(null); setExpired(false); setMessage(''); setReceipt(null);
    if (current.current) void cancelPortalReview(current.current.session_id);
    current.current = null;
    try {
      const value = await preparePortalReview(nextSource, text, redact);
      if (!active.current || sequence.current !== request) { void cancelPortalReview(value.session_id); return; }
      current.current = value; setPreview(value);
    } catch { if (active.current && sequence.current === request) setMessage('Review unavailable. Nothing was sent.'); }
    finally { if (active.current && sequence.current === request) setBusy(false); }
  };
  const dispose = () => {
    active.current = false; sequence.current++;
    if (current.current) void cancelPortalReview(current.current.session_id);
    current.current = null;
    if (sending.current) void cancelPortalReview(sending.current);
    sending.current = null;
  };
  useEffect(() => {
    active.current = true;
    const previous = document.activeElement as HTMLElement | null;
    dialog.current?.showModal(); cancel.current?.focus();
    void refresh(initialSource);
    return () => { dispose(); previous?.focus(); };
    // Preparation is explicit after editing; draft changes never silently approve.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    if (!preview) return;
    const timer = window.setTimeout(() => {
      setExpired(true);
      if (current.current) void cancelPortalReview(current.current.session_id);
      current.current = null;
    }, 60_000);
    return () => window.clearTimeout(timer);
  }, [preview]);
  const confirm = async () => {
    const p = current.current;
    if (!p || busy || expired || text !== p.text || redact !== p.redact || !p.can_confirm_offline) return;
    current.current = null; setBusy(true);
    try {
      const receipt = await finishPortalReview(p.session_id, p.fingerprint, false);
      if (active.current) { setMessage(receipt === 'offline_reviewed' ? 'Offline review confirmed. Nothing was sent.' : 'Review unavailable. Nothing was sent.'); setExpired(true); }
    } catch { if (active.current) { setMessage('Review expired or unavailable. Nothing was sent.'); setExpired(true); } }
    finally { if (active.current) setBusy(false); }
  };
  const send = async () => {
    const p = current.current;
    if (!p || !p.can_send || busy || expired || text !== p.text || redact !== p.redact) return;
    current.current = null; sending.current = p.session_id; setBusy(true); setReceipt(null);
    setMessage('Waiting for native Portal consent…');
    try {
      const value = await sendPortalReview(p.session_id, p.fingerprint);
      if (active.current) { setReceipt(value); setMessage(value.status === 'settled' ? 'Remote review completed.' : `Remote review: ${value.code || value.status}.`); setExpired(true); }
    } catch { if (active.current) { setMessage('Native host unavailable. Billing outcome unknown. No retry was made.'); setExpired(true); } }
    finally { sending.current = null; if (active.current) setBusy(false); }
  };
  return <dialog ref={dialog} aria-labelledby="portal-review-title" onCancel={e => { e.preventDefault(); onClose(); }}
    className="m-auto w-[min(42rem,95vw)] max-h-[90vh] overflow-y-auto rounded-2xl border border-zinc-600 bg-zinc-950 p-6 text-zinc-100 backdrop:bg-black/80">
    <h2 id="portal-review-title" className="text-xl font-semibold">Review remote image request</h2>
    <p className="mt-2 text-sm text-zinc-300">dreamcatcher · classify-image</p>
    <p className="break-all text-xs text-zinc-400">google/gemini-3.5-flash-lite · google-vertex/us</p>
    <p className="mt-3 text-sm">Remote vision requires an operator-enabled Portal route and native consent. Personal media cannot be sent.</p>
    <div className="my-4 flex gap-3">
      <button type="button" onClick={() => { setSource('generated'); void refresh('generated'); }} disabled={busy}
        className="rounded border border-zinc-600 p-2">Use generated example</button>
      <button type="button" onClick={() => { setSource('selection'); void refresh('selection'); }} disabled={busy}
        className="rounded border border-zinc-600 p-2">Check selected-image route</button>
    </div>
    {source === 'generated' && <>
      <label className="block text-sm">Text to review
        <textarea value={text} onChange={e => setText(e.target.value)} rows={3}
          className="mt-1 block w-full rounded border border-zinc-600 bg-zinc-900 p-2" /></label>
      <label className="my-2 flex gap-2 text-sm"><input type="checkbox" checked={redact} onChange={e => setRedact(e.target.checked)} />Redact the top-left square</label>
      <button type="button" disabled={busy} onClick={() => void refresh('generated')} className="rounded border border-zinc-600 p-2 text-sm">Update preview</button>
    </>}
    {preview?.image && <figure className="my-4">
      <img src={preview.image} alt="Exact sanitized generated image, including selected redaction" width="192" height="192" style={{ imageRendering: 'pixelated' }} className="mx-auto" />
      <figcaption className="mt-2 text-xs">Exact prepared text</figcaption>
      <pre className="max-h-36 overflow-auto whitespace-pre-wrap break-words rounded bg-zinc-900 p-3 text-sm">{preview.text}</pre>
      <p className="text-xs">One RGB PNG · 32 × 32 pixels · Output limit: {preview.max_tokens} tokens</p>
    </figure>}
    <p role="status" className="my-4 text-sm text-amber-200">{message || (busy ? 'Preparing review…' : expired ? 'Review expired or used. Update the preview to review again.' : preview?.message)}</p>
    {receipt && <section aria-label="Remote review receipt" className="my-3 rounded border border-zinc-700 p-3 text-sm">
      <p>{receipt.model} · {receipt.endpoint}</p>
      <p>Billing: {receipt.billing_state}{receipt.cost_usd_micros != null ? ` · $${(receipt.cost_usd_micros / 1_000_000).toFixed(6)} · ${receipt.tokens} tokens` : ' · cost unknown'}</p>
      {receipt.classification && <pre className="mt-2 whitespace-pre-wrap break-words">{receipt.classification}</pre>}
    </section>}
    <div className="flex flex-wrap justify-end gap-3">
      <button ref={cancel} type="button" onClick={onClose} className="rounded border border-zinc-500 px-4 py-2">Cancel</button>
      <button type="button" disabled={busy || expired || !preview?.can_confirm_offline || text !== preview.text || redact !== preview.redact}
        onClick={() => void confirm()} className="rounded border border-cyan-500 px-4 py-2 disabled:opacity-40">Confirm offline review once</button>
      <button type="button" disabled={busy || expired || !preview?.can_send || text !== preview.text || redact !== preview.redact}
        onClick={() => void send()} className="rounded border border-zinc-700 px-4 py-2 disabled:opacity-40">Review and send remotely</button>
    </div>
  </dialog>;
}
export function PortalReviewQualification() {
  const [open, setOpen] = useState(true);
  return <main className="min-h-screen bg-zinc-950 p-8 text-zinc-100"><h1 className="text-xl">DreamCatcher · generated-image consent qualification</h1>
    <p>No media library, settings, classifier or external transport is loaded in this view.</p>
    <button className="mt-4 rounded border p-3" onClick={() => setOpen(true)}>Open review</button>
    {open && <PortalImageReview initialSource="generated" onClose={() => setOpen(false)} />}
  </main>;
}
