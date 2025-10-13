const { invoke, convertFileSrc } = window.__TAURI__.tauri;
const fsApi = (window.__TAURI__ && window.__TAURI__.fs) ? window.__TAURI__.fs : null;

class WhisperApp {
    constructor() {
        this.currentAudioPath = null;
        this.currentModelId = null;
        this.audioDuration = 0;
        this.isTranscribing = false;
        this.isEditMode = false;
        this.isClickPlay = false;
        this.selectedLanguage = 'ja';
        this.translateToEnglish = false;
        this.useGpu = false;
        this.useRemoteServer = false;
        this.serverUrl = '';
        this.serverEndpoint = '';
        this.currentWhisperThreads = null;
        this.maxThreads = null;

        this.audio = new Audio();
        this.audio.preload = 'auto';
        this.audio.autoplay = false; // 自動再生を無効化
        this._loadingAudio = false;
        this._blobUrl = null;
        this._triedBlobFallback = false;
        this._playbackPath = null; // 実際に再生に使うファイルパス（プレビュー等）
        this._bindAudioEvents();
        this._bindDownloadEvents();
        this._bindTaskProgressEvents();

        this.initializeElements();
        this.attachEventListeners();
        this.loadAvailableModels();
        this.loadGpuSettings();
        this.loadPerformanceSettings();
        this.addLog('準備完了');
    }

    initializeElements() {
        this.audioPathInput = document.getElementById('audio-path-input');
        this.browseButton = document.getElementById('browse-button');
        this.loadButton = document.getElementById('load-button');
        this.mediaControls = document.getElementById('media-controls');
        this.progressSlider = document.getElementById('progress-slider');
        this.playPauseBtn = document.getElementById('play-pause-btn');
        this.transcriptionRange = document.getElementById('transcription-range');
        this.startTimeInput = document.getElementById('start-time');
        this.endTimeInput = document.getElementById('end-time');
        this.startRangeSlider = document.getElementById('start-range');
        this.endRangeSlider = document.getElementById('end-range');
        this.modelPathInput = document.getElementById('model-path-input');
        this.browseModelButton = document.getElementById('browse-model-button');
        this.updateModelButton = document.getElementById('update-model-button');
        this.modelSelect = document.getElementById('model-select');
        this.switchModelButton = document.getElementById('switch-model-button');
        this.downloadModelButton = document.getElementById('download-model-button');
        this.downloadAllButton = document.getElementById('download-all-button');
        this.downloadProgressRow = document.getElementById('download-progress-row');
        this.downloadProgress = document.getElementById('download-progress');
        this.downloadProgressText = document.getElementById('download-progress-text');
        this.fileReadProgressRow = document.getElementById('file-read-progress-row');
        this.fileReadProgress = document.getElementById('file-read-progress');
        this.fileReadProgressText = document.getElementById('file-read-progress-text');
        this.taskProgressRow = document.getElementById('task-progress-row');
        this.taskProgress = document.getElementById('task-progress');
        this.taskProgressText = document.getElementById('task-progress-text');
        this.languageSelect = document.getElementById('language-select');
        this.translateToggle = document.getElementById('translate-toggle');
        this.useGpuToggle = document.getElementById('use-gpu-toggle');
        this.useGpuServerToggle = document.getElementById('use-gpu-server-toggle');
        this.serverUrlInput = document.getElementById('server-url-input');
        this.startTranscriptionBtn = document.getElementById('start-transcription');
        this.editModeToggle = document.getElementById('edit-mode-toggle');
        this.clickPlayToggle = document.getElementById('click-play-toggle');
        this.copyResultsBtn = document.getElementById('copy-results');
        this.clearResultsBtn = document.getElementById('clear-results');
        this.transcriptionResults = document.getElementById('transcription-results');
        this.logContainer = document.getElementById('log-container');
        this.serverEndpointInput = document.getElementById('server-endpoint-input');
        this.cpuThreadsInput = document.getElementById('cpu-threads-input');
        this.cpuThreadsMaxEl = document.getElementById('cpu-threads-max');
    }

    // 音声の読み込み中は操作を無効化
    setUiLoadingAudio(loading) {
        this._loadingAudio = !!loading;
        try {
            if (this.playPauseBtn) this.playPauseBtn.disabled = !!loading;
            if (this.startTranscriptionBtn && !this.isTranscribing) {
                this.startTranscriptionBtn.disabled = !!loading;
            }
        } catch (_) {}
    }

    _bindDownloadEvents() {
        try {
            const eventApi = window.__TAURI__ && window.__TAURI__.event;
            if (!eventApi || typeof eventApi.listen !== 'function') return;
            eventApi.listen('download-progress', (event) => {
                const p = event && event.payload ? event.payload : {};
                const id = p.id || '-';
                const fn = p.filename || '';
                const downloaded = p.downloaded || 0;
                const total = (p.total !== undefined && p.total !== null) ? p.total : null;
                const phase = p.phase || 'progress';
                const msg = p.message || '';

                // 表示を確実に出す
                if (this.downloadProgressRow) this.downloadProgressRow.style.display = 'block';

                const toMB = (v) => (v / (1024 * 1024)).toFixed(1);
                let percent = null;
                if (total && total > 0) {
                    percent = Math.max(0, Math.min(100, Math.floor((downloaded / total) * 100)));
                }

                if (phase === 'start') {
                    if (this.downloadProgress) this.downloadProgress.value = 0;
                    if (this.downloadProgressText) {
                        const totalTxt = total ? `${toMB(total)} MB` : 'サイズ不明';
                        this.downloadProgressText.textContent = `開始: ${fn} (${totalTxt})`;
                    }
                    this.addLog(`ダウンロード開始: ${id} (${fn})`);
                } else if (phase === 'progress') {
                    if (this.downloadProgress && percent !== null) this.downloadProgress.value = percent;
                    if (this.downloadProgressText) {
                        const base = total ? `${toMB(downloaded)} / ${toMB(total)} MB` : `${toMB(downloaded)} MB`;
                        const pctTxt = percent !== null ? ` (${percent}%)` : '';
                        this.downloadProgressText.textContent = `${fn}: ${base}${pctTxt}`;
                    }
                } else if (phase === 'done') {
                    if (this.downloadProgress) this.downloadProgress.value = 100;
                    if (this.downloadProgressText) this.downloadProgressText.textContent = `完了: ${fn}`;
                    this.addLog(`ダウンロード完了: ${fn}`);
                } else if (phase === 'error') {
                    if (this.downloadProgressText) this.downloadProgressText.textContent = `エラー: ${fn} ${msg}`;
                    this.addLog(`ダウンロードエラー: ${fn} ${msg}`);
                }
            });
        } catch (_) {
            // ignore
        }
    }

    _bindTaskProgressEvents() {
        try {
            const eventApi = window.__TAURI__ && window.__TAURI__.event;
            if (!eventApi || typeof eventApi.listen !== 'function') return;
            eventApi.listen('task-progress', (event) => {
                const p = event && event.payload ? event.payload : {};
                const task = p.task || '';
                const filename = p.filename || '';
                const done = Number(p.done || 0);
                const total = (p.total !== undefined && p.total !== null) ? Number(p.total) : null;
                const phase = p.phase || 'progress';
                const msg = p.message || '';
                const isFilePreview = (task === 'file-preview');
                const isFileRead = (task === 'file-read');

                // ターゲットUIを決定
                let rowEl, barEl, textEl, label;
                if (isFilePreview || isFileRead) {
                    rowEl = this.fileReadProgressRow;
                    barEl = this.fileReadProgress;
                    textEl = this.fileReadProgressText;
                    label = isFilePreview ? '読み込み(プレビュー)' : '読み込み';
                } else {
                    rowEl = this.taskProgressRow;
                    barEl = this.taskProgress;
                    textEl = this.taskProgressText;
                    if (task === 'upload') label = 'GPUサーバへ送信';
                    else label = '処理中';
                }

                if (rowEl) rowEl.style.display = 'block';

                let percent = null;
                if (total && total > 0) {
                    percent = Math.max(0, Math.min(100, Math.floor((done / total) * 100)));
                }

                if (phase === 'start') {
                    if (barEl) barEl.value = 0;
                    if (textEl) textEl.textContent = `${label} 開始 ${filename ? '(' + filename + ')' : ''}`;
                } else if (phase === 'progress') {
                    if (barEl && percent !== null) barEl.value = percent;
                    if (textEl) {
                        if (isFileRead || isFilePreview) {
                            // ファイル読み込みはサンプル数ベースの進捗なので％表示のみ
                            if (percent !== null) {
                                textEl.textContent = `${label} ${percent}% ${filename ? '(' + filename + ')' : ''}`;
                            } else {
                                textEl.textContent = `${label} 進行中 ${filename ? '(' + filename + ')' : ''}`;
                            }
                        } else {
                            if (percent !== null) {
                                const doneMB = (done / (1024*1024)).toFixed(1);
                                const totalMB = total ? (total / (1024*1024)).toFixed(1) : '-';
                                textEl.textContent = `${label} ${percent}% (${doneMB}/${totalMB} MB) ${filename ? '(' + filename + ')' : ''}`;
                            } else {
                                textEl.textContent = `${label} ${done} / ? ${filename ? '(' + filename + ')' : ''}`;
                            }
                        }
                    }
                } else if (phase === 'done') {
                    if (isFilePreview) {
                        if (barEl) barEl.value = 100;
                        if (textEl) textEl.textContent = `読み込み完了 ${filename ? '(' + filename + ')' : ''}`;
                        setTimeout(() => { if (rowEl) rowEl.style.display = 'none'; }, 1500);
                    } else if (isFileRead) {
                        // 読み込み完了後は「文字おこし中」を表示し続ける
                        if (barEl) { try { barEl.removeAttribute('value'); } catch (_) {} }
                        if (textEl) textEl.textContent = `文字おこし中`;
                    } else {
                        if (barEl) barEl.value = 100;
                        if (textEl) textEl.textContent = `${label} 完了 ${filename ? '(' + filename + ')' : ''}`;
                        setTimeout(() => { if (rowEl) rowEl.style.display = 'none'; }, 2000);
                    }
                } else if (phase === 'error') {
                    if (textEl) textEl.textContent = `${label} エラー: ${msg}`;
                    // 4秒後に隠す
                    setTimeout(() => { if (rowEl) rowEl.style.display = 'none'; }, 4000);
                }
            });
        } catch (_) {}
    }

    attachEventListeners() {
        this.browseButton.addEventListener('click', () => this.browseAudioFile());
        this.loadButton.addEventListener('click', () => this.loadAudioFile());
        this.browseModelButton.addEventListener('click', () => this.browseModelFile());
        this.updateModelButton.addEventListener('click', () => this.updateModelPath());
        this.switchModelButton.addEventListener('click', () => this.switchModel());
        if (this.downloadModelButton) {
            this.downloadModelButton.addEventListener('click', () => this.downloadSelectedModel());
        }
        if (this.downloadAllButton) {
            this.downloadAllButton.addEventListener('click', () => this.downloadAllModels());
        }
        this.languageSelect.addEventListener('change', (e) => this.changeLanguage(e.target.value));
        this.translateToggle.addEventListener('change', (e) => this.toggleTranslation(e.target.checked));
        if (this.useGpuToggle) {
            this.useGpuToggle.addEventListener('change', async (e) => {
                this.useGpu = !!e.target.checked;
                await this.updateGpuSettings();
                this.addLog(`GPU: ${this.useGpu ? 'ON' : 'OFF'}`);
            });
        }
        if (this.useGpuServerToggle) {
            this.useGpuServerToggle.addEventListener('change', async (e) => {
                this.useRemoteServer = !!e.target.checked;
                await this.updateGpuSettings();
                this.applyRemoteUiState();
                this.addLog(`GPUサーバ: ${this.useRemoteServer ? 'ON' : 'OFF'}`);
            });
        }
        if (this.serverUrlInput) {
            const handler = async () => {
                this.serverUrl = (this.serverUrlInput.value || '').trim();
                await this.updateRemoteServerSettings();
                this.addLog(`GPUサーバURLを更新: ${this.serverUrl || '(未設定)'}`);
            };
            this.serverUrlInput.addEventListener('change', handler);
            this.serverUrlInput.addEventListener('blur', handler);
        }
        if (this.serverEndpointInput) {
            const handlerEp = async () => {
                this.serverEndpoint = (this.serverEndpointInput.value || '').trim();
                await this.updateRemoteServerSettings();
                this.addLog(`エンドポイントを更新: ${this.serverEndpoint || '(未設定)'}`);
            };
            this.serverEndpointInput.addEventListener('change', handlerEp);
            this.serverEndpointInput.addEventListener('blur', handlerEp);
        }
        this.startTranscriptionBtn.addEventListener('click', () => this.startTranscription());
        this.editModeToggle.addEventListener('change', (e) => this.toggleEditMode(e.target.checked));
        this.clickPlayToggle.addEventListener('change', (e) => this.toggleClickPlay(e.target.checked));
        this.copyResultsBtn.addEventListener('click', () => this.copyResults());
        if (this.clearResultsBtn) {
            this.clearResultsBtn.addEventListener('click', () => this.clearResults());
        }

        // メディアコントロール
        this.playPauseBtn.addEventListener('click', () => this.togglePlayPause());
        this.progressSlider.addEventListener('input', (e) => this.seekAudio(e.target.value));

        // 結果テキストのクリック再生
        this.transcriptionResults.addEventListener('click', (e) => this.onResultsClick(e));

        // 文字起こし範囲スライダー
        this.startRangeSlider.addEventListener('input', (e) => this.updateStartTime(e.target.value));
        this.endRangeSlider.addEventListener('input', (e) => this.updateEndTime(e.target.value));

        // CPU スレッド数の変更
        if (this.cpuThreadsInput) {
            const handlerThreads = async () => {
                let v = Number(this.cpuThreadsInput.value || 0);
                if (!Number.isFinite(v) || v <= 0) v = 1;
                const max = this.maxThreads || 1;
                const clamped = Math.max(1, Math.min(max, Math.floor(v)));
                if (clamped !== v) {
                    this.cpuThreadsInput.value = String(clamped);
                }
                try {
                    await invoke('update_whisper_threads', { threads: clamped });
                    this.currentWhisperThreads = clamped;
                    this.addLog(`使用CPU数を ${clamped} に設定しました`);
                } catch (e) {
                    this.addLog(`CPU数の更新に失敗しました: ${e}`);
                }
            };
            this.cpuThreadsInput.addEventListener('change', handlerThreads);
            this.cpuThreadsInput.addEventListener('blur', handlerThreads);
        }
    }

    async loadPerformanceSettings() {
        try {
            const info = await invoke('get_performance_info');
            const wt = (info && typeof info.whisperThreads !== 'undefined') ? info.whisperThreads : info.whisper_threads;
            const mt = (info && typeof info.maxThreads !== 'undefined') ? info.maxThreads : info.max_threads;
            this.currentWhisperThreads = Number(wt) || 1;
            this.maxThreads = Number(mt) || 1;
            if (this.cpuThreadsInput) {
                this.cpuThreadsInput.min = 1;
                this.cpuThreadsInput.max = String(this.maxThreads);
                this.cpuThreadsInput.value = String(this.currentWhisperThreads);
            }
            if (this.cpuThreadsMaxEl) {
                this.cpuThreadsMaxEl.textContent = `最大: ${this.maxThreads}`;
            }
            this.addLog(`CPU情報: 現在 ${this.currentWhisperThreads} / 最大 ${this.maxThreads}`);
        } catch (e) {
            this.addLog(`CPU情報の取得に失敗しました: ${e}`);
        }
    }

    async browseAudioFile() {
        try {
            const selected = await invoke('select_audio_file');
            if (selected) {
                this.audioPathInput.value = selected;
                this.currentAudioPath = selected;
                this.addLog(`音声ファイルを選択: ${selected}`);
                await this.loadAudioFile();
            }
        } catch (error) {
            this.addLog(`ファイル選択エラー: ${error}`);
        }
    }

    async loadAudioFile() {
        const path = this.audioPathInput.value.trim();
        if (!path) {
            this.addLog('音声ファイルパスを入力してください');
            return;
        }

        try {
            // 読み込み開始時は操作を無効化
            this.setUiLoadingAudio(true);
            const metadata = await invoke('load_audio_metadata', { path });
            this.currentAudioPath = path;
            this.showMediaControls(metadata);
            // 再生用にWAVプレビューを生成（ブラウザのコーデック差異を回避）
            try {
                const previewPath = await invoke('prepare_preview_wav', { path });
                this.setAudioSource(previewPath);
                this.addLog('プレビュー音声を準備しました');
            } catch (e) {
                // 失敗時は元ファイルにフォールバック
                this.setAudioSource(path);
                this.addLog(`プレビュー生成に失敗: 元ファイルで再生を試みます (${e})`);
            }
            this.addLog(`音声ファイルを読み込みました: ${this.formatDuration(metadata.duration)}`);
        } catch (error) {
            this.addLog(`音声読み込みエラー: ${error}`);
            // 失敗時は操作を戻す
            this.setUiLoadingAudio(false);
        }
    }

    showMediaControls(metadata) {
        this.mediaControls.style.display = 'block';
        this.transcriptionRange.style.display = 'block';
        this.audioDuration = metadata.duration;
        this.updateTimeDisplay(metadata.duration);
        this.initializeRangeSliders();
    }

    initializeRangeSliders() {
        this.startRangeSlider.max = 100;
        this.endRangeSlider.max = 100;
        this.startRangeSlider.value = 0;
        this.endRangeSlider.value = 100;
        this.updateStartTime(0);
        this.updateEndTime(100);
    }

    updateStartTime(value) {
        const timeInSeconds = (value / 100) * this.audioDuration;
        this.startTimeInput.value = this.formatDuration(timeInSeconds);
    }

    updateEndTime(value) {
        const timeInSeconds = (value / 100) * this.audioDuration;
        this.endTimeInput.value = this.formatDuration(timeInSeconds);
    }

    changeLanguage(language) {
        this.selectedLanguage = language;
        const languageMap = {
            'auto': '自動検出',
            'ja': '日本語',
            'en': 'English',
            'zh': '中文',
            'ko': '한국어'
        };
        this.addLog(`言語設定: ${languageMap[language] || language}`);

        invoke('update_language_setting', { language }).catch(error => {
            this.addLog(`言語設定エラー: ${error}`);
        });
    }

    async loadAvailableModels() {
        try {
            const models = await invoke('get_available_models');
            this.populateModelSelect(models);
        } catch (error) {
            this.addLog(`モデル一覧の取得に失敗しました: ${error}`);
        }
    }

    populateModelSelect(models) {
        this.modelSelect.innerHTML = '<option value="">未選択</option>';
        models.forEach(model => {
            const option = document.createElement('option');
            option.value = model.id;
            option.textContent = `${model.label}${model.downloaded ? '' : ' (未ダウンロード)'}`;
            if (model.current) {
                option.selected = true;
                this.currentModelId = model.id;
                this.modelPathInput.value = model.path || model.filename;
            }
            this.modelSelect.appendChild(option);
        });
    }

    async browseModelFile() {
        try {
            const selected = await invoke('select_model_file');
            if (selected) {
                this.modelPathInput.value = selected;
                this.addLog(`モデルファイルを選択: ${selected}`);
            }
        } catch (error) {
            this.addLog(`モデルファイル選択エラー: ${error}`);
        }
    }

    updateModelPath() {
        const path = this.modelPathInput.value.trim();
        if (!path) {
            this.addLog('モデルファイルパスを入力してください');
            return;
        }
        this.addLog(`モデルパスを更新: ${path}`);
    }

    async switchModel() {
        const modelId = this.modelSelect.value;
        if (!modelId) {
            this.addLog('モデルを選択してください');
            return;
        }

        try {
            const result = await invoke('select_model', { modelId });
            this.currentModelId = modelId;
            this.addLog(result);
            await this.loadAvailableModels(); // リストを更新
        } catch (error) {
            this.addLog(`モデル切替エラー: ${error}`);
        }
    }

    async downloadSelectedModel() {
        const modelId = this.modelSelect.value;
        if (!modelId) {
            this.addLog('モデルを選択してください');
            return;
        }
        try {
            this.addLog(`ダウンロード開始: ${modelId}`);
            if (this.downloadModelButton) this.downloadModelButton.disabled = true;
            if (this.downloadAllButton) this.downloadAllButton.disabled = true;
            const msg = await invoke('download_model', { modelId });
            this.addLog(msg);
            await this.loadAvailableModels();
        } catch (error) {
            this.addLog(`モデルダウンロードに失敗しました: ${error}`);
        } finally {
            if (this.downloadModelButton) this.downloadModelButton.disabled = false;
            if (this.downloadAllButton) this.downloadAllButton.disabled = false;
        }
    }

    async downloadAllModels() {
        try {
            this.addLog('未ダウンロードモデルの一括ダウンロードを開始します');
            if (this.downloadModelButton) this.downloadModelButton.disabled = true;
            if (this.downloadAllButton) this.downloadAllButton.disabled = true;
            const list = await invoke('download_all_models');
            if (Array.isArray(list) && list.length > 0) {
                this.addLog(`まとめてダウンロード完了: ${list.join(', ')}`);
            } else {
                this.addLog('ダウンロード対象はありません（全て揃っています）');
            }
            await this.loadAvailableModels();
        } catch (error) {
            this.addLog(`まとめてダウンロードに失敗しました: ${error}`);
        } finally {
            if (this.downloadModelButton) this.downloadModelButton.disabled = false;
            if (this.downloadAllButton) this.downloadAllButton.disabled = false;
        }
    }

    toggleTranslation(enabled) {
        this.translateToEnglish = enabled;
        this.addLog(`英語翻訳: ${enabled ? 'ON' : 'OFF'}`);
    }

    async startTranscription() {
        if (!this.currentAudioPath) {
            this.addLog('音声ファイルが選択されていません');
            return;
        }

        if (this.useRemoteServer) {
            const url = (this.serverUrl || '').trim();
            if (!url) {
                this.addLog('GPUサーバURLを入力してください (例: http://192.168.0.1:8080)');
                return;
            }
            const ep = (this.serverEndpoint || '').trim();
            if (!ep) {
                this.addLog('エンドポイントを入力してください (例: /transcribe-with-timestamps)');
                return;
            }
        }

        if (this.isTranscribing) {
            this.addLog('既に文字起こし中です');
            return;
        }

        this.isTranscribing = true;
        this.startTranscriptionBtn.disabled = true;
        this.startTranscriptionBtn.textContent = '文字起こし中...';

        this.addLog('文字起こしを開始します');

        try {
            const result = await invoke('start_transcription', {
                audioPath: this.currentAudioPath,
                language: this.selectedLanguage,
                translateToEnglish: this.translateToEnglish
            });

            this.transcriptionResults.value = result.text;
            this.addLog(`文字起こしが完了しました (${result.segments || 0} セグメント)`);
        } catch (error) {
            this.addLog(`文字起こしエラー: ${error}`);
        } finally {
            this.isTranscribing = false;
            this.startTranscriptionBtn.disabled = false;
            this.startTranscriptionBtn.textContent = '文字起こし開始';
            // 推移中のタスク進捗（読み込み/アップロード表示）を閉じる
            if (this.taskProgressRow) this.taskProgressRow.style.display = 'none';
            if (this.taskProgress) try { this.taskProgress.value = 0; } catch (_) {}
            if (this.taskProgressText) this.taskProgressText.textContent = '-';
            if (this.fileReadProgressRow) this.fileReadProgressRow.style.display = 'none';
            if (this.fileReadProgress) try { this.fileReadProgress.value = 0; } catch (_) {}
            if (this.fileReadProgressText) this.fileReadProgressText.textContent = '-';
        }
    }

    // 旧: 別ウィンドウ起動。タブ化したため未使用。

    async loadGpuSettings() {
        try {
            const s = await invoke('get_gpu_settings');
            if (s && typeof s === 'object') {
                this.useGpu = !!s.useGpu || !!s.use_gpu;
                this.useRemoteServer = !!s.useRemoteServer || !!s.use_remote_server;
                this.serverUrl = s.remoteServerUrl || s.remote_server_url || '';
                this.serverEndpoint = s.remoteServerEndpoint || s.remote_server_endpoint || '';
            }
        } catch (_) {
            // フォールバック: 旧コマンド
            try {
                const s = await invoke('get_remote_server_settings');
                if (s && typeof s === 'object') {
                    this.useGpu = !!s.useGpu || !!s.use_gpu;
                    this.useRemoteServer = !!s.useRemoteServer || !!s.use_remote_server;
                    this.serverUrl = s.remoteServerUrl || s.remote_server_url || '';
                    this.serverEndpoint = s.remoteServerEndpoint || s.remote_server_endpoint || '';
                }
            } catch (_) {}
        }
        // 反映
        if (this.useGpuToggle) this.useGpuToggle.checked = !!this.useGpu;
        if (this.useGpuServerToggle) this.useGpuServerToggle.checked = !!this.useRemoteServer;
        if (this.serverUrlInput) this.serverUrlInput.value = this.serverUrl || '';
        if (this.serverEndpointInput) this.serverEndpointInput.value = this.serverEndpoint || '';
        this.applyRemoteUiState();
    }

    async updateGpuSettings() {
        try {
            await invoke('update_gpu_settings', {
                useGpu: !!this.useGpu,
                useRemoteServer: !!this.useRemoteServer,
                remoteServerUrl: (this.serverUrl || '').trim(),
                remoteServerEndpoint: (this.serverEndpoint || '').trim()
            });
        } catch (error) {
            // フォールバック: 旧コマンド
            try {
                await invoke('update_remote_server_settings', {
                    useRemoteServer: !!this.useRemoteServer,
                    remoteServerUrl: (this.serverUrl || '').trim(),
                    remoteServerEndpoint: (this.serverEndpoint || '').trim()
                });
            } catch (_) {}
            this.addLog(`GPU設定の更新に失敗しました: ${error}`);
        }
    }

    async loadRemoteServerSettings() {
        // 旧メソッド名との互換性のため残す
        await this.loadGpuSettings();
    }

    async updateRemoteServerSettings() {
        // 旧メソッド名との互換性のため残す
        await this.updateGpuSettings();
    }

    applyRemoteUiState() {
        const disabled = !!this.useRemoteServer;
        const widgets = [
            this.modelSelect,
            this.switchModelButton,
            this.downloadModelButton,
            this.downloadAllButton,
            this.browseModelButton,
            this.updateModelButton,
            this.modelPathInput
        ];
        widgets.forEach(w => { if (w) w.disabled = disabled; });
        if (this.serverUrlInput) this.serverUrlInput.disabled = false; // 入力は常に可能
        if (this.serverEndpointInput) this.serverEndpointInput.disabled = false; // 入力は常に可能
    }

    toggleEditMode(enabled) {
        this.isEditMode = enabled;
        this.transcriptionResults.readOnly = !enabled;
        this.addLog(`編集モード: ${enabled ? 'ON' : 'OFF'}`);
    }

    toggleClickPlay(enabled) {
        this.isClickPlay = enabled;
        this.addLog(`クリック再生: ${enabled ? 'ON' : 'OFF'}`);
    }

    async copyResults() {
        const text = this.transcriptionResults.value;
        if (!text.trim()) {
            this.addLog('コピーする結果がありません');
            return;
        }

        try {
            await invoke('copy_to_clipboard', { text });
            this.addLog('解析結果をクリップボードにコピーしました');
        } catch (error) {
            // フォールバック: ブラウザのClipboard API
            try {
                await navigator.clipboard.writeText(text);
                this.addLog('解析結果をクリップボードにコピーしました');
            } catch (clipboardError) {
                this.addLog('クリップボードへのコピーに失敗しました');
            }
        }
    }

    clearResults() {
        if (!this.transcriptionResults.value.trim()) {
            this.addLog('結果は既に空です');
            return;
        }
        this.transcriptionResults.value = '';
        this.addLog('解析結果をクリアしました');
    }

    togglePlayPause() {
        if (!this.audio || !this.audio.src) {
            this.addLog('再生できる音声がありません');
            return;
        }
        if (this.audio.paused) {
            this.audio.play()
                .then(() => {
                    this.playPauseBtn.textContent = '⏸';
                    this.addLog('再生を開始しました');
                })
                .catch((err) => {
                    this.addLog(`再生失敗: ${err && err.message ? err.message : err}`);
                });
        } else {
            this.audio.pause();
            this.playPauseBtn.textContent = '▶';
            this.addLog('再生を停止しました');
        }
    }

    seekAudio(value) {
        if (!this.audioDuration || !this.audio) return;
        const pct = Math.max(0, Math.min(100, Number(value)));
        const t = (pct / 100) * this.audioDuration;
        this.audio.currentTime = t;
    }

    setAudioSource(path) {
        try {
            let url;
            if (typeof convertFileSrc === 'function') {
                url = convertFileSrc(path);
            } else if (window.__TAURI__ && typeof window.__TAURI__.convertFileSrc === 'function') {
                url = window.__TAURI__.convertFileSrc(path);
            } else {
                url = path; // 最低限のフォールバック
            }
            // 既存の Blob URL を解放
            if (this._blobUrl) {
                try { URL.revokeObjectURL(this._blobUrl); } catch (_) {}
                this._blobUrl = null;
            }
            this._triedBlobFallback = false;
            this._playbackPath = path;

            // 新しいソースの読み込みが始まるので無効化
            this.setUiLoadingAudio(true);
            this.audio.src = url;
            try { this.audio.load(); } catch (_) {}
            this.addLog(`audio: src set -> ${url}`);
            this.playPauseBtn.textContent = '▶';
        } catch (_) {
            this.addLog('音声の読み込みURL生成に失敗しました');
        }
    }

    _bindAudioEvents() {
        this.audio.addEventListener('loadedmetadata', () => {
            if (isFinite(this.audio.duration)) {
                this.audioDuration = this.audio.duration;
                this.updateTimeDisplay(this.audioDuration);
                this.progressSlider.value = 0;
            }
        });
        // 再生可能になったら操作を有効化
        const onReady = () => {
            if (this._loadingAudio) {
                this.setUiLoadingAudio(false);
                this.addLog('audio: ready');
            }
        };
        this.audio.addEventListener('loadeddata', onReady);
        this.audio.addEventListener('canplay', onReady);
        this.audio.addEventListener('canplaythrough', onReady);
        this.audio.addEventListener('timeupdate', () => {
            if (!this.audioDuration) return;
            const cur = this.audio.currentTime;
            const pct = (cur / this.audioDuration) * 100;
            this.progressSlider.value = Math.max(0, Math.min(100, pct));
            const currentTimeEl = document.getElementById('current-time');
            if (currentTimeEl) currentTimeEl.textContent = this.formatDuration(cur);
        });
        this.audio.addEventListener('ended', () => {
            this.playPauseBtn.textContent = '▶';
        });
        this.audio.addEventListener('play', () => {
            this.addLog('audio: play');
        });
        this.audio.addEventListener('pause', () => {
            this.addLog('audio: pause');
        });
        this.audio.addEventListener('seeking', () => {
            this.addLog(`audio: seeking -> ${this.formatDuration(this.audio.currentTime || 0)}`);
        });
        this.audio.addEventListener('seeked', () => {
            this.addLog(`audio: seeked -> ${this.formatDuration(this.audio.currentTime || 0)}`);
        });
        this.audio.addEventListener('waiting', () => {
            this.addLog('audio: waiting');
        });
        this.audio.addEventListener('stalled', () => {
            this.addLog('audio: stalled');
        });
        this.audio.addEventListener('error', () => {
            const err = this.audio.error;
            const codes = { 1: 'ABORTED', 2: 'NETWORK', 3: 'DECODE', 4: 'SRC_NOT_SUPPORTED' };
            const msg = err ? `code=${err.code} (${codes[err.code] || 'UNKNOWN'})` : 'unknown';
            this.addLog(`audio: error ${msg}`);
            // ファイルURLの再生に失敗した場合、Blob URL フォールバックを試す
            if (err && err.code === 4 && !this._triedBlobFallback) {
                this._triedBlobFallback = true;
                this.addLog('audio: Blob URL にフォールバックを試みます');
                this._loadViaBlob()
                    .then((ok) => {
                        if (ok) {
                            this.addLog('audio: Blob フォールバックを設定しました');
                        } else {
                            // フォールバックも失敗
                            this.setUiLoadingAudio(false);
                        }
                    })
                    .catch((e) => {
                        this.addLog(`Blob フォールバックに失敗: ${e}`);
                        this.setUiLoadingAudio(false);
                    });
            }
            // エラー時は一旦操作可能に戻す（ユーザーに再試行させる）
            if (!this._triedBlobFallback) {
                this.setUiLoadingAudio(false);
            }
        });

        // ファイルから読み込んで Blob URL として設定
        this._loadViaBlob = async () => {
            try {
                const p = this._playbackPath || this.currentAudioPath;
                if (!p) return false;
                if (!fsApi || typeof fsApi.readBinaryFile !== 'function') {
                    this.addLog('fs.readBinaryFile が利用できません');
                    return false;
                }
                const mime = this._guessMimeFromPath(p);
                const bytes = await fsApi.readBinaryFile(p);
                const u8 = (bytes instanceof Uint8Array) ? bytes : new Uint8Array(bytes);
                const blob = new Blob([u8], { type: mime });
                if (this._blobUrl) {
                    try { URL.revokeObjectURL(this._blobUrl); } catch (_) {}
                    this._blobUrl = null;
                }
                this._blobUrl = URL.createObjectURL(blob);
                this.audio.src = this._blobUrl;
                try { this.audio.load(); } catch (_) {}
                this.addLog(`audio: src set (blob) -> ${mime}`);
                // 自動再生しない（ユーザー操作でのみ再生）
                this.playPauseBtn.textContent = '▶';
                return true;
            } catch (e) {
                this.addLog(`Blob 生成エラー: ${e}`);
                return false;
            }
        };

        // 拡張子から簡易的に MIME を推定
        this._guessMimeFromPath = (p) => {
            const lower = (p || '').toLowerCase();
            if (lower.endsWith('.wav')) return 'audio/wav';
            if (lower.endsWith('.mp3')) return 'audio/mpeg';
            if (lower.endsWith('.m4a') || lower.endsWith('.mp4') || lower.endsWith('.aac')) return 'audio/mp4';
            if (lower.endsWith('.ogg') || lower.endsWith('.oga')) return 'audio/ogg';
            if (lower.endsWith('.flac')) return 'audio/flac';
            return 'audio/*';
        };
    }

    onResultsClick(e) {
        if (!this.isClickPlay) return;
        const text = this.transcriptionResults.value || '';
        if (!text) return;

        const pos = this.transcriptionResults.selectionStart || 0;
        const lineStart = text.lastIndexOf('\n', Math.max(0, pos - 1)) + 1;
        const nextNl = text.indexOf('\n', pos);
        const lineEnd = nextNl === -1 ? text.length : nextNl;
        const line = text.slice(lineStart, lineEnd);

        const m = line.match(/^\s*\[(\d{2}):(\d{2}):(\d{2})(?:[\.,](\d{2,3}))?\s*-->/);
        if (!m) {
            this.addLog('クリック位置の行にタイムスタンプが見つかりません');
            return;
        }
        const h = parseInt(m[1], 10) || 0;
        const min = parseInt(m[2], 10) || 0;
        const s = parseInt(m[3], 10) || 0;
        const ms = parseInt(m[4] || '0', 10) || 0;
        const sec = h * 3600 + min * 60 + s + ms / 1000;
        if (!this.audio || !this.audio.src) {
            this.addLog('音声が読み込まれていません');
            return;
        }
        if (isFinite(sec)) {
            this.audio.currentTime = Math.max(0, Math.min(this.audioDuration || sec, sec));
            this.audio.play()
                .then(() => {
                    this.playPauseBtn.textContent = '⏸';
                    this.addLog(`クリック再生: ${this.formatDuration(sec)}`);
                })
                .catch((err) => {
                    this.addLog(`クリック再生失敗: ${err && err.message ? err.message : err}`);
                });
        }
    }

    addLog(message) {
        const timestamp = new Date().toLocaleString('ja-JP', {
            year: 'numeric',
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
        });

        const logEntry = document.createElement('div');
        logEntry.className = 'log-entry';
        logEntry.innerHTML = `
            <span class="timestamp">[${timestamp}]</span>
            <span class="message">${message}</span>
        `;

        this.logContainer.appendChild(logEntry);
        this.logContainer.scrollTop = this.logContainer.scrollHeight;

        // ログエントリーが多くなりすぎた場合の制限
        const maxEntries = 100;
        while (this.logContainer.children.length > maxEntries) {
            this.logContainer.removeChild(this.logContainer.firstChild);
        }
    }

    updateTimeDisplay(duration) {
        const currentTimeEl = document.getElementById('current-time');
        const totalTimeEl = document.getElementById('total-time');

        if (totalTimeEl) {
            totalTimeEl.textContent = this.formatDuration(duration);
        }
        if (currentTimeEl) {
            currentTimeEl.textContent = '0:00';
        }
    }

    formatDuration(seconds) {
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = Math.floor(seconds % 60);

        if (hours > 0) {
            return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
        } else {
            return `${minutes}:${secs.toString().padStart(2, '0')}`;
        }
    }
}

// アプリケーション初期化
document.addEventListener('DOMContentLoaded', () => {
    new WhisperApp();
});
