const { invoke } = window.__TAURI__.tauri;

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

        this.initializeElements();
        this.attachEventListeners();
        this.loadAvailableModels();
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
        this.languageSelect = document.getElementById('language-select');
        this.translateToggle = document.getElementById('translate-toggle');
        this.startTranscriptionBtn = document.getElementById('start-transcription');
        this.editModeToggle = document.getElementById('edit-mode-toggle');
        this.clickPlayToggle = document.getElementById('click-play-toggle');
        this.copyResultsBtn = document.getElementById('copy-results');
        this.clearResultsBtn = document.getElementById('clear-results');
        this.transcriptionResults = document.getElementById('transcription-results');
        this.logContainer = document.getElementById('log-container');
    }

    attachEventListeners() {
        this.browseButton.addEventListener('click', () => this.browseAudioFile());
        this.loadButton.addEventListener('click', () => this.loadAudioFile());
        this.browseModelButton.addEventListener('click', () => this.browseModelFile());
        this.updateModelButton.addEventListener('click', () => this.updateModelPath());
        this.switchModelButton.addEventListener('click', () => this.switchModel());
        this.languageSelect.addEventListener('change', (e) => this.changeLanguage(e.target.value));
        this.translateToggle.addEventListener('change', (e) => this.toggleTranslation(e.target.checked));
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

        // 文字起こし範囲スライダー
        this.startRangeSlider.addEventListener('input', (e) => this.updateStartTime(e.target.value));
        this.endRangeSlider.addEventListener('input', (e) => this.updateEndTime(e.target.value));
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
            const metadata = await invoke('load_audio_metadata', { path });
            this.currentAudioPath = path;
            this.showMediaControls(metadata);
            this.addLog(`音声ファイルを読み込みました: ${this.formatDuration(metadata.duration)}`);
        } catch (error) {
            this.addLog(`音声読み込みエラー: ${error}`);
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

    toggleTranslation(enabled) {
        this.translateToEnglish = enabled;
        this.addLog(`英語翻訳: ${enabled ? 'ON' : 'OFF'}`);
    }

    async startTranscription() {
        if (!this.currentAudioPath) {
            this.addLog('音声ファイルが選択されていません');
            return;
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
        }
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
        // 実際の音声再生機能は後で実装
        const isPlaying = this.playPauseBtn.textContent === '⏸';
        this.playPauseBtn.textContent = isPlaying ? '▶' : '⏸';
        this.addLog(isPlaying ? '再生を停止しました' : '再生を開始しました');
    }

    seekAudio(value) {
        // 実際のシーク機能は後で実装
        this.addLog(`シーク: ${value}%`);
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
