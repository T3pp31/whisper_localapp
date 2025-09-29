class WhisperWebUI {
    constructor() {
        this.uploadArea = document.getElementById('upload-area');
        this.fileInput = document.getElementById('file-input');
        this.uploadProgress = document.getElementById('upload-progress');
        this.progressFill = document.getElementById('progress-fill');
        this.progressText = document.getElementById('progress-text');
        this.resultsSection = document.getElementById('results-section');
        this.languageSelect = document.getElementById('language-select');

        this.currentFile = null;
        this.init();
    }

    init() {
        this.setupEventListeners();
        this.loadInitialData();
    }

    setupEventListeners() {
        this.uploadArea.addEventListener('click', () => this.fileInput.click());
        this.fileInput.addEventListener('change', (e) => this.handleFileSelect(e.target.files[0]));

        this.uploadArea.addEventListener('dragover', (e) => this.handleDragOver(e));
        this.uploadArea.addEventListener('dragleave', (e) => this.handleDragLeave(e));
        this.uploadArea.addEventListener('drop', (e) => this.handleDrop(e));

        document.getElementById('copy-text-btn').addEventListener('click', () => this.copyText());
        document.getElementById('download-text-btn').addEventListener('click', () => this.downloadText());
        document.getElementById('download-json-btn').addEventListener('click', () => this.downloadJSON());
        document.getElementById('clear-results-btn').addEventListener('click', () => this.clearResults());

        document.getElementById('notification-close').addEventListener('click', () => this.hideNotification());
    }

    async loadInitialData() {
        await Promise.all([
            this.checkBackendHealth(),
            this.loadLanguages(),
            this.loadServerInfo(),
            this.loadStats()
        ]);
    }

    async checkBackendHealth() {
        try {
            const response = await fetch('/api/health');
            const data = await response.json();

            const statusEl = document.getElementById('backend-status');
            if (data.success && data.data.status === 'healthy') {
                statusEl.textContent = 'オンライン';
                statusEl.className = 'status-value online';
            } else {
                statusEl.textContent = 'オフライン';
                statusEl.className = 'status-value offline';
            }

            await this.checkGPUStatus();
        } catch (error) {
            document.getElementById('backend-status').textContent = 'エラー';
            document.getElementById('backend-status').className = 'status-value offline';
            console.error('Backend health check failed:', error);
        }
    }

    async checkGPUStatus() {
        try {
            const response = await fetch('/api/gpu-status');
            const data = await response.json();

            const gpuStatusEl = document.getElementById('gpu-status');
            if (data.success && data.data.gpu_available) {
                gpuStatusEl.textContent = `${data.data.gpu_name || 'GPU'}利用可能`;
                gpuStatusEl.className = 'status-value online';
            } else {
                gpuStatusEl.textContent = 'CPU';
                gpuStatusEl.className = 'status-value';
            }
        } catch (error) {
            document.getElementById('gpu-status').textContent = '不明';
            console.error('GPU status check failed:', error);
        }
    }

    async loadLanguages() {
        try {
            const response = await fetch('/api/languages');
            const data = await response.json();

            if (data.success && data.data.languages) {
                const select = this.languageSelect;
                select.innerHTML = '<option value="">自動検出</option>';

                data.data.languages.forEach(lang => {
                    const option = document.createElement('option');
                    option.value = lang.code;
                    option.textContent = `${lang.name} (${lang.code})`;
                    select.appendChild(option);
                });
            }
        } catch (error) {
            console.error('Failed to load languages:', error);
        }
    }

    async loadServerInfo() {
        try {
            const response = await fetch('/api/health');
            const data = await response.json();

            if (data.success) {
                const info = data.data;
                document.getElementById('server-info').innerHTML = `
                    <div>ステータス: ${info.status}</div>
                    <div>稼働時間: ${Math.floor(info.uptime_seconds / 3600)}時間 ${Math.floor((info.uptime_seconds % 3600) / 60)}分</div>
                    <div>Whisperエンジン: ${info.whisper_loaded ? '読み込み済み' : '未読み込み'}</div>
                    <div>バージョン: ${info.version || 'N/A'}</div>
                `;
            }
        } catch (error) {
            document.getElementById('server-info').innerHTML = 'サーバー情報の取得に失敗しました';
        }
    }

    async loadStats() {
        try {
            const response = await fetch('/api/stats');
            const data = await response.json();

            if (data.success) {
                const stats = data.data;
                const successRate = stats.requests_total > 0 ?
                    ((stats.requests_successful / stats.requests_total) * 100).toFixed(1) : 0;

                document.getElementById('stats-info').innerHTML = `
                    <div>総リクエスト数: ${stats.requests_total}</div>
                    <div>成功: ${stats.requests_successful}</div>
                    <div>失敗: ${stats.requests_failed}</div>
                    <div>成功率: ${successRate}%</div>
                    ${stats.average_processing_time ?
                        `<div>平均処理時間: ${stats.average_processing_time.toFixed(2)}秒</div>` : ''
                    }
                `;
            }
        } catch (error) {
            document.getElementById('stats-info').innerHTML = '統計情報の取得に失敗しました';
        }
    }

    handleDragOver(e) {
        e.preventDefault();
        this.uploadArea.classList.add('drag-over');
    }

    handleDragLeave(e) {
        e.preventDefault();
        if (!this.uploadArea.contains(e.relatedTarget)) {
            this.uploadArea.classList.remove('drag-over');
        }
    }

    handleDrop(e) {
        e.preventDefault();
        this.uploadArea.classList.remove('drag-over');
        const files = e.dataTransfer.files;
        if (files.length > 0) {
            this.handleFileSelect(files[0]);
        }
    }

    handleFileSelect(file) {
        if (!file) return;

        const allowedTypes = [
            'audio/wav', 'audio/mpeg', 'audio/mp4', 'audio/flac', 'audio/ogg',
            'video/mp4', 'video/quicktime', 'video/x-msvideo', 'video/x-matroska'
        ];

        if (!allowedTypes.some(type => file.type.startsWith(type.split('/')[0]))) {
            this.showNotification('サポートされていないファイル形式です', 'error');
            return;
        }

        this.currentFile = file;
        this.showNotification(`ファイル選択: ${file.name}`, 'success');
        this.uploadFile();
    }

    async uploadFile() {
        if (!this.currentFile) return;

        const formData = new FormData();
        formData.append('file', this.currentFile);

        const language = this.languageSelect.value;
        if (language) formData.append('language', language);

        const withTimestamps = document.getElementById('with-timestamps').checked;
        formData.append('with_timestamps', withTimestamps.toString());

        const temperature = document.getElementById('temperature').value;
        if (temperature) formData.append('temperature', temperature);

        const noSpeechThreshold = document.getElementById('no-speech-threshold').value;
        if (noSpeechThreshold) formData.append('no_speech_threshold', noSpeechThreshold);

        this.showProgress('アップロード中...');

        try {
            const response = await fetch('/api/upload', {
                method: 'POST',
                body: formData
            });

            const result = await response.json();

            if (result.success) {
                this.displayResults(result.data, withTimestamps);
                this.showNotification('文字起こしが完了しました', 'success');
            } else {
                throw new Error(result.error || '文字起こしに失敗しました');
            }
        } catch (error) {
            this.showNotification(`エラー: ${error.message}`, 'error');
            console.error('Upload failed:', error);
        } finally {
            this.hideProgress();
            await this.loadStats();
        }
    }

    displayResults(data, withTimestamps) {
        document.getElementById('result-text').textContent = data.text;
        document.getElementById('processing-time').textContent =
            data.processing_time ? data.processing_time.toFixed(2) : 'N/A';
        document.getElementById('audio-duration').textContent =
            data.duration ? data.duration.toFixed(2) : 'N/A';
        document.getElementById('detected-language').textContent =
            data.language || '不明';

        if (withTimestamps && data.segments) {
            this.displaySegments(data.segments);
            document.getElementById('segments-container').style.display = 'block';
        } else {
            document.getElementById('segments-container').style.display = 'none';
        }

        this.resultsSection.style.display = 'block';
        this.currentResultData = data;
    }

    displaySegments(segments) {
        const container = document.getElementById('segments');
        container.innerHTML = '';

        segments.forEach(segment => {
            const segmentEl = document.createElement('div');
            segmentEl.className = 'segment';

            const timeEl = document.createElement('div');
            timeEl.className = 'segment-time';
            timeEl.textContent = `${this.formatTime(segment.start)} - ${this.formatTime(segment.end)}`;

            const textEl = document.createElement('div');
            textEl.className = 'segment-text';
            textEl.textContent = segment.text;

            segmentEl.appendChild(timeEl);
            segmentEl.appendChild(textEl);
            container.appendChild(segmentEl);
        });
    }

    formatTime(seconds) {
        const mins = Math.floor(seconds / 60);
        const secs = Math.floor(seconds % 60);
        return `${mins}:${secs.toString().padStart(2, '0')}`;
    }

    showProgress(text) {
        document.querySelector('.upload-content').style.display = 'none';
        this.uploadProgress.style.display = 'block';
        this.progressText.textContent = text;
        this.progressFill.style.width = '100%';
    }

    hideProgress() {
        document.querySelector('.upload-content').style.display = 'block';
        this.uploadProgress.style.display = 'none';
        this.progressFill.style.width = '0%';
    }

    showNotification(message, type = 'info') {
        const notification = document.getElementById('notification');
        const notificationText = document.getElementById('notification-text');

        notificationText.textContent = message;
        notification.className = `notification ${type}`;
        notification.style.display = 'flex';

        setTimeout(() => this.hideNotification(), 5000);
    }

    hideNotification() {
        document.getElementById('notification').style.display = 'none';
    }

    async copyText() {
        const text = document.getElementById('result-text').textContent;
        try {
            await navigator.clipboard.writeText(text);
            this.showNotification('テキストをクリップボードにコピーしました', 'success');
        } catch (error) {
            this.showNotification('コピーに失敗しました', 'error');
        }
    }

    downloadText() {
        const text = document.getElementById('result-text').textContent;
        const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
        this.downloadBlob(blob, 'transcription.txt');
    }

    downloadJSON() {
        if (!this.currentResultData) return;

        const blob = new Blob([JSON.stringify(this.currentResultData, null, 2)],
            { type: 'application/json;charset=utf-8' });
        this.downloadBlob(blob, 'transcription.json');
    }

    downloadBlob(blob, filename) {
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
    }

    clearResults() {
        this.resultsSection.style.display = 'none';
        this.currentResultData = null;
        this.currentFile = null;
        this.fileInput.value = '';
    }
}

document.addEventListener('DOMContentLoaded', () => {
    new WhisperWebUI();
});