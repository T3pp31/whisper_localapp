/* global window */
(function(){
  const { tauri, event } = window.__TAURI__ || {};
  const invoke = tauri && tauri.invoke ? tauri.invoke : async ()=>{ throw new Error('Tauri not available'); };

  const el = (id)=>document.getElementById(id);
  const $start = el('rt-start');
  const $stop = el('rt-stop');
  const $status = el('rt-status');
  const $vu = el('rt-vu');
  const $vuText = el('rt-vu-text');
  const $partial = el('rt-partial');
  const $final = el('rt-final');

  const setStatus = (txt)=>{ $status.textContent = txt; };
  const setRunning = (running)=>{
    $start.disabled = !!running;
    $stop.disabled = !running;
  };

  const onRealtimeStatus = (p)=>{
    const phase = (p && p.phase) || 'unknown';
    const msg = (p && p.message) ? `: ${p.message}` : '';
    setStatus(`${phase}${msg}`);
    setRunning(phase === 'running');
  };
  const onRealtimeLevel = (p)=>{
    const peak = Math.max(0, Math.min(1, Number((p && p.peak) || 0)));
    $vu.style.width = `${Math.floor(peak * 100)}%`;
    const db = (p && typeof p.rms === 'number') ? (20*Math.log10(Math.max(1e-6, p.rms))).toFixed(1) : '-';
    $vuText.textContent = `${db} dB`;
  };
  const onRealtimeText = (p)=>{
    const kind = (p && p.kind) || 'partial';
    const text = (p && p.text) || '';
    if (kind === 'partial') {
      $partial.textContent = text;
    } else if (kind === 'final') {
      $partial.textContent = '';
      const cur = $final.value || '';
      $final.value = cur ? (cur + '\n' + text) : text;
      $final.scrollTop = $final.scrollHeight;
    }
  };

  async function initListeners(){
    try {
      if (!event || typeof event.listen !== 'function') return;
      await event.listen('realtime-status', (e)=> onRealtimeStatus(e && e.payload));
      await event.listen('realtime-level', (e)=> onRealtimeLevel(e && e.payload));
      await event.listen('realtime-text', (e)=> onRealtimeText(e && e.payload));
      // 初期ステータス取得
      try {
        const s = await invoke('realtime_status');
        onRealtimeStatus(s);
      } catch(_){}
    } catch(_){}
  }

  $start.addEventListener('click', async ()=>{
    try {
      setStatus('starting');
      await invoke('realtime_start', { device: null, language: null });
    } catch(e) {
      setStatus('error: ' + (e && e.toString ? e.toString() : e));
      setRunning(false);
    }
  });
  $stop.addEventListener('click', async ()=>{
    try {
      await invoke('realtime_stop');
    } catch(e) {
      setStatus('error: ' + (e && e.toString ? e.toString() : e));
    }
  });

  initListeners();
})();

