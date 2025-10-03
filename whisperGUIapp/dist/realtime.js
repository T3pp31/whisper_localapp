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
  const $device = el('rt-device');
  const $model = el('rt-model');
  const $browseModel = el('rt-browse-model');
  const $language = el('rt-language');
  const $threads = el('rt-threads');
  const $threadsMax = el('rt-threads-max');
  const $switchModel = el('rt-switch-model');
  let selectedModelPath = null; // 参照で直接指定した場合に使用

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

  async function populateDevices(){
    if (!$device) return;
    $device.innerHTML = '';
    const optDefault = document.createElement('option');
    optDefault.value = '';
    optDefault.textContent = '既定のデバイス';
    $device.appendChild(optDefault);
    try {
      const names = await invoke('list_input_devices');
      if (Array.isArray(names)) {
        for (const name of names) {
          const opt = document.createElement('option');
          opt.value = name;
          opt.textContent = name;
          $device.appendChild(opt);
        }
      }
    } catch (e) {
      // ignore, keep default option
    }
  }

  async function populateModels(){
    if (!$model) return;
    $model.innerHTML = '';
    const optNone = document.createElement('option');
    optNone.value = '';
    optNone.textContent = '未選択 (現在の設定)';
    $model.appendChild(optNone);
    try {
      const models = await invoke('get_available_models');
      if (Array.isArray(models)) {
        for (const m of models) {
          const opt = document.createElement('option');
          opt.value = m.id;
          opt.textContent = `${m.label}${m.downloaded ? '' : ' (未ダウンロード)'}`;
          $model.appendChild(opt);
          if (m.current) {
            $model.value = m.id;
          }
        }
      }
    } catch(e) {
      // ignore
    }
    // モデル選択時は参照指定をクリア
    $model.addEventListener('change', ()=>{ selectedModelPath = null; });
  }

  async function initThreads(){
    if (!$threads) return;
    try {
      const info = await invoke('get_performance_info');
      const wt = (info && (info.whisperThreads ?? info.whisper_threads)) || 1;
      const mt = (info && (info.maxThreads ?? info.max_threads)) || 1;
      $threads.value = String(wt);
      $threads.min = '1';
      $threads.max = String(mt);
      if ($threadsMax) $threadsMax.textContent = `(最大 ${mt})`;
    } catch(e) {
      $threads.value = '4';
      if ($threadsMax) $threadsMax.textContent = '';
    }
  }

  $start.addEventListener('click', async ()=>{
    try {
      setStatus('starting');
      const device = ($device && $device.value) ? $device.value : null;
      const language = ($language && $language.value) ? $language.value : null;
      const t = $threads ? parseInt($threads.value, 10) : NaN;
      const threads = Number.isFinite(t) && t > 0 ? t : null;
      const model_id = ($model && $model.value) ? ($model.value || null) : null;
      const model_path = selectedModelPath;
      await invoke('realtime_start', { device, language, threads, model_id, model_path });
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
  populateDevices();
  populateModels();
  initThreads();

  if ($browseModel) {
    $browseModel.addEventListener('click', async ()=>{
      try {
        const p = await invoke('select_model_file');
        if (p) {
          selectedModelPath = p;
          if ($model) $model.value = '';
        }
      } catch(e) {
        // ignore
      }
    });
  }

  if ($switchModel) {
    $switchModel.addEventListener('click', async ()=>{
      try {
        if ($model && $model.value) {
          const modelId = $model.value;
          const msg = await invoke('select_model', { modelId });
          setStatus(msg || 'switched');
          await populateModels();
        } else if (selectedModelPath) {
          setStatus('モデルIDを選択してください（参照はリアルタイムのみ適用）');
        } else {
          setStatus('モデルが未選択です');
        }
      } catch(e) {
        setStatus('error: ' + (e && e.toString ? e.toString() : e));
      }
    });
  }
})();
