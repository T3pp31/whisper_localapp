class WhisperWebUI {
    constructor() {
        this.uploadArea = document.getElementById('upload-area');
        this.fileInput = document.getElementById('file-input');
        this.uploadProgress = document.getElementById('upload-progress');
        this.progressFill = document.getElementById('progress-fill');
        this.progressText = document.getElementById('progress-text');
        this.uploadText = document.getElementById('upload-text');
        this.uploadStatus = document.getElementById('upload-status');
        this.uploadPreviewContainer = document.getElementById('upload-preview');
        this.uploadPreviewAudio = document.getElementById('upload-audio-preview');
        this.defaultUploadText = this.uploadText?.dataset.defaultText?.trim()
            || this.uploadText?.textContent?.trim()
            || '';
        this.uploadSuccessTemplate = this.uploadStatus?.dataset.successText?.trim() || '';
        this.resultsSection = document.getElementById('results-section');
        this.languageSelect = document.getElementById('language-select');
        this.audioContainer = document.getElementById('audio-player-container');
        this.audioPlayer = document.getElementById('audio-player');
        this.timelineContainer = document.getElementById('timeline-container');
        this.timeline = document.getElementById('timeline');
        this.timelineProgress = document.getElementById('timeline-progress');
        this.timelineSegmentsContainer = document.getElementById('timeline-segments');
        this.notificationCloseBtn = document.getElementById('notification-close');
        this.transcribeButton = document.getElementById('transcribe-btn');
        this.transcribeButtonLabel = this.transcribeButton?.dataset.label?.trim() || this.transcribeButton?.textContent || '';
        this.transcribeButtonLoadingLabel = this.transcribeButton?.dataset.loadingLabel?.trim() || '文字起こし中...';

        this.appConfig = document.getElementById('app-config');
        const configDefaultLanguage = this.appConfig?.dataset.defaultLanguage?.trim();
        const configTimelineUpdate = Number(this.appConfig?.dataset.timelineUpdateMs);
        const configStatsAverageLabel = this.appConfig?.dataset.statsAverageProcessingTimeLabel?.trim();
        const configStatsAverageUnit = this.appConfig?.dataset.statsAverageProcessingTimeUnit;
        this.defaultLanguage = configDefaultLanguage || '';
        this.timelineUpdateInterval = Number.isFinite(configTimelineUpdate) && configTimelineUpdate > 0
            ? configTimelineUpdate
            : 200;
        this.statsAverageProcessingTimeLabel = configStatsAverageLabel || '平均処理時間 (音声1分あたりの文字起こし所要時間)';
        this.statsAverageProcessingTimeUnit =
            typeof configStatsAverageUnit === 'string' && configStatsAverageUnit.trim().length > 0
                ? configStatsAverageUnit.trim()
                : '秒 / 音声1分';

        this.currentFile = null;
        this.currentResultData = null;
        this.audioUrl = null;
        this.segmentElements = [];
        this.currentSegments = [];
        this.timelineData = { segments: [], duration: 0 };
        this.lastTimelineUpdate = 0;
        this.isUploading = false;

        this.init();
    }

    init() {
        this.setupEventListeners();
        this.applyDefaultLanguageOption();
        this.loadInitialData();
        this.updateTranscribeButtonState();
    }

    setupEventListeners() {
        if (this.uploadArea) {
            this.uploadArea.addEventListener('click', () => this.fileInput?.click());
            this.uploadArea.addEventListener('dragover', (e) => this.handleDragOver(e));
            this.uploadArea.addEventListener('dragleave', (e) => this.handleDragLeave(e));
            this.uploadArea.addEventListener('drop', (e) => this.handleDrop(e));
        }

        if (this.fileInput) {
            this.fileInput.addEventListener('change', (e) => this.handleFileSelect(e.target.files[0]));
        }

        document.getElementById('copy-text-btn')?.addEventListener('click', () => this.copyText());
        document.getElementById('download-text-btn')?.addEventListener('click', () => this.downloadText());
        document.getElementById('download-json-btn')?.addEventListener('click', () => this.downloadJSON());
        document.getElementById('clear-results-btn')?.addEventListener('click', () => this.clearResults());

        this.notificationCloseBtn?.addEventListener('click', () => this.hideNotification());

        if (this.transcribeButton) {
            this.transcribeButton.addEventListener('click', () => this.handleTranscribeRequest());
        }

        if (this.timelineContainer) {
            this.timelineContainer.addEventListener('click', (event) => this.handleTimelineClick(event));
        }

        if (this.audioPlayer) {
            this.audioPlayer.addEventListener('timeupdate', () => this.handleTimeUpdate());
            this.audioPlayer.addEventListener('loadedmetadata', () => this.handleLoadedMetadata());
            this.audioPlayer.addEventListener('ended', () => this.clearActiveSegment());
        }
    }

    async loadInitialData() {
        await Promise.all([
            this.checkBackendHealth(),
            this.loadLanguages(),
            this.loadServerInfo(),
            this.loadStats(),
        ]);
    }

    async checkBackendHealth() {
        const statusEl = document.getElementById('backend-status');
        if (!statusEl) {
            return;
        }

        try {
            const response = await fetch('/api/health');
            const data = await response.json();

            if (data.success && data.data) {
                const backendStatus = (data.data.status || '').toString().toLowerCase();
                const isHealthy = backendStatus === 'healthy' || backendStatus === 'ok';
                statusEl.textContent = isHealthy ? 'オンライン' : 'オフライン';
                statusEl.className = `status-value ${isHealthy ? 'online' : 'offline'}`;
            } else {
                statusEl.textContent = 'オフライン';
                statusEl.className = 'status-value offline';
            }

            await this.checkGPUStatus();
        } catch (error) {
            statusEl.textContent = 'エラー';
            statusEl.className = 'status-value offline';
            console.error('Backend health check failed:', error);
            await this.checkGPUStatus();
        }
    }

    async checkGPUStatus() {
        const gpuStatusEl = document.getElementById('gpu-status');
        if (!gpuStatusEl) {
            return;
        }
        const gpuContainer = gpuStatusEl.parentElement;
        if (!gpuContainer) {
            return;
        }

        const showContainer = () => {
            gpuContainer.style.display = '';
        };

        try {
            const response = await fetch('/api/gpu-status');
            const data = await response.json();

            if (data.success && data.data) {
                showContainer();
                if (data.data.gpu_available) {
                    const label = data.data.gpu_name ? `${data.data.gpu_name}利用中` : 'GPU利用中';
                    gpuStatusEl.textContent = label;
                    gpuStatusEl.className = 'status-value online';
                } else {
                    gpuStatusEl.textContent = data.data.gpu_enabled_in_config ? 'GPU未使用' : 'GPU未設定';
                    gpuStatusEl.className = 'status-value';
                }
            } else {
                showContainer();
                gpuStatusEl.textContent = '情報取得に失敗';
                gpuStatusEl.className = 'status-value offline';
            }
        } catch (error) {
            showContainer();
            gpuStatusEl.textContent = '情報取得に失敗';
            gpuStatusEl.className = 'status-value offline';
            console.error('GPU status check failed:', error);
        }
    }

    async loadLanguages() {
        try {
            const response = await fetch('/api/languages');
            const data = await response.json();

            if (data.success && data.data.languages && this.languageSelect) {
                this.languageSelect.innerHTML = '<option value="">自動検出</option>';

                data.data.languages.forEach((lang) => {
                    const option = document.createElement('option');
                    option.value = lang.code;
                    option.textContent = `${lang.name} (${lang.code})`;
                    this.languageSelect.appendChild(option);
                });
            }
        } catch (error) {
            console.error('Failed to load languages:', error);
        } finally {
            this.applyDefaultLanguageOption();
        }
    }

    applyDefaultLanguageOption() {
        if (!this.languageSelect || !this.defaultLanguage) {
            return;
        }

        const hasDefault = Array.from(this.languageSelect.options).some(
            (option) => option.value === this.defaultLanguage,
        );

        if (!hasDefault) {
            const option = document.createElement('option');
            option.value = this.defaultLanguage;
            option.textContent = `${this.defaultLanguage} (設定)`;
            this.languageSelect.appendChild(option);
        }

        this.languageSelect.value = this.defaultLanguage;
    }

    async loadServerInfo() {
        const container = document.getElementById('server-info');
        if (!container) {
            return;
        }

        try {
            const response = await fetch('/api/health');
            const data = await response.json();

            if (data.success && data.data) {
                const info = data.data;
                const uptimeSeconds = Number(info.uptime_seconds) || 0;
                const hours = Math.floor(uptimeSeconds / 3600);
                const minutes = Math.floor((uptimeSeconds % 3600) / 60);
                const versionText = info.version ?? 'N/A';
                const parts = [
                    `<div>ステータス: ${info.status || '不明'}</div>`,
                    `<div>稼働時間: ${hours}時間 ${minutes}分</div>`,
                    `<div>Whisperエンジン: ${info.whisper_loaded ? '読み込み済み' : '未読み込み'}</div>`,
                    `<div>バージョン: ${versionText}</div>`,
                ];

                if (info.memory_usage_mb !== null && info.memory_usage_mb !== undefined) {
                    parts.push(`<div>メモリ使用量: ${info.memory_usage_mb}MB</div>`);
                }

                container.innerHTML = parts.join('');
            } else {
                const message = data && data.error
                    ? `サーバー情報の取得に失敗しました: ${data.error}`
                    : 'サーバー情報の取得に失敗しました';
                container.textContent = message;
            }
        } catch (error) {
            container.textContent = 'サーバー情報の取得に失敗しました';
            console.error('Failed to load server info:', error);
        }
    }

    async loadStats() {
        const container = document.getElementById('stats-info');
        if (!container) {
            return;
        }

        try {
            const response = await fetch('/api/stats');
            const data = await response.json();

            if (data.success && data.data) {
                const stats = data.data;
                const total = Number(stats.requests_total) || 0;
                const successful = Number(stats.requests_successful) || 0;
                const failed = Number(stats.requests_failed) || 0;
                const successRate = total > 0
                    ? ((successful / total) * 100).toFixed(1)
                    : '0.0';

                const parts = [
                    `<div>総リクエスト数: ${total}</div>`,
                    `<div>成功: ${successful}</div>`,
                    `<div>失敗: ${failed}</div>`,
                    `<div>成功率: ${successRate}%</div>`,
                ];

                if (typeof stats.average_processing_time === 'number' && stats.average_processing_time > 0) {
                    const label = this.escapeHtml(this.statsAverageProcessingTimeLabel || '');
                    const unit = this.escapeHtml(this.statsAverageProcessingTimeUnit || '');
                    const formattedValue = `${stats.average_processing_time.toFixed(2)}${unit ? ` ${unit}` : ''}`;
                    const labelPrefix = label ? `${label}: ` : '';
                    parts.push(`<div>${labelPrefix}${formattedValue}</div>`);
                }

                if (typeof stats.active_requests === 'number') {
                    parts.push(`<div>稼働中リクエスト: ${stats.active_requests}</div>`);
                }

                container.innerHTML = parts.join('');
            } else {
                const message = data && data.error
                    ? `統計情報の取得に失敗しました: ${data.error}`
                    : '統計情報の取得に失敗しました';
                container.textContent = message;
            }
        } catch (error) {
            container.textContent = '統計情報の取得に失敗しました';
            console.error('Failed to load stats:', error);
        }
    }

    escapeHtml(text) {
        if (typeof text !== 'string') {
            return '';
        }

        return text
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    }

    handleDragOver(event) {
        event.preventDefault();
        this.uploadArea?.classList.add('drag-over');
    }

    handleDragLeave(event) {
        event.preventDefault();
        if (!this.uploadArea) return;
        if (!this.uploadArea.contains(event.relatedTarget)) {
            this.uploadArea.classList.remove('drag-over');
        }
    }

    handleDrop(event) {
        event.preventDefault();
        this.uploadArea?.classList.remove('drag-over');
        const files = event.dataTransfer.files;
        if (files.length > 0) {
            this.handleFileSelect(files[0]);
        }
    }

    setUploadState(fileName) {
        if (!this.uploadArea || !this.uploadText) {
            return;
        }

        if (fileName) {
            this.uploadText.textContent = fileName;
            this.uploadText.classList.add('uploaded');
            this.uploadArea.classList.add('has-file');

            if (this.uploadStatus) {
                this.uploadStatus.textContent = this.formatUploadSuccessMessage(fileName);
                this.uploadStatus.style.display = 'block';
            }

            if (this.uploadPreviewContainer && this.uploadPreviewAudio?.src) {
                this.uploadPreviewContainer.style.display = 'block';
            }

            return;
        }

        this.uploadText.textContent = this.defaultUploadText;
        this.uploadText.classList.remove('uploaded');
        this.uploadArea.classList.remove('has-file');

        if (this.uploadStatus) {
            this.uploadStatus.style.display = 'none';
            this.uploadStatus.textContent = '';
        }

        if (this.uploadPreviewContainer && this.uploadPreviewAudio) {
            this.uploadPreviewContainer.style.display = 'none';
            this.uploadPreviewAudio.pause();
            this.uploadPreviewAudio.removeAttribute('src');
            this.uploadPreviewAudio.load();
        }
    }

    formatUploadSuccessMessage(fileName) {
        if (!fileName) {
            return '';
        }

        if (!this.uploadSuccessTemplate) {
            return `アップロード準備完了: ${fileName}`;
        }

        if (this.uploadSuccessTemplate.includes('{filename}')) {
            return this.uploadSuccessTemplate.split('{filename}').join(fileName);
        }

        return `${this.uploadSuccessTemplate} ${fileName}`;
    }

    handleFileSelect(file) {
        if (!file) return;

        if (!this.isFileAllowed(file)) {
            this.showNotification('サポートされていないファイル形式です', 'error');
            if (!this.currentFile) {
                this.setUploadState(null);
            }
            return;
        }

        this.currentFile = file;
        this.prepareAudio(file);
        this.setUploadState(file.name);
        this.showNotification(`ファイル選択: ${file.name}。文字起こしボタンをクリックして開始してください`, 'success');
        this.updateTranscribeButtonState();
    }

    handleTranscribeRequest() {
        if (!this.currentFile) {
            this.showNotification('音声ファイルを選択してください', 'error');
            return;
        }

        if (this.isUploading) {
            return;
        }

        void this.uploadFile();
    }

    isFileAllowed(file) {
        if (!file) return false;
        if (file.type) {
            const typeRoot = file.type.split('/')[0];
            if (typeRoot === 'audio' || typeRoot === 'video') {
                return true;
            }
        }

        const extension = file.name?.split('.').pop()?.toLowerCase();
        if (!extension || !this.fileInput) {
            return false;
        }

        const acceptAttr = this.fileInput.getAttribute('accept') || '';
        return acceptAttr
            .split(',')
            .map((value) => value.trim().replace('.', '').toLowerCase())
            .some((value) => value === extension);
    }

    prepareAudio(file) {
        if (!this.audioPlayer) return;

        if (this.audioUrl) {
            URL.revokeObjectURL(this.audioUrl);
        }

        this.audioUrl = URL.createObjectURL(file);
        this.audioPlayer.src = this.audioUrl;
        this.audioPlayer.currentTime = 0;
        if (this.uploadPreviewAudio) {
            this.uploadPreviewAudio.src = this.audioUrl;
            this.uploadPreviewAudio.currentTime = 0;
            if (this.uploadPreviewContainer) {
                this.uploadPreviewContainer.style.display = 'block';
            }
        }
        this.timelineProgress && (this.timelineProgress.style.width = '0%');
        this.lastTimelineUpdate = 0;
        this.updateAudioAvailability();
    }

    async uploadFile() {
        if (!this.currentFile) {
            this.showNotification('音声ファイルを選択してください', 'error');
            return;
        }

        if (this.isUploading) {
            return;
        }

        this.isUploading = true;
        this.updateTranscribeButtonState();

        const formData = new FormData();
        formData.append('file', this.currentFile);

        const language = this.languageSelect?.value;
        if (language) formData.append('language', language);

        const withTimestamps = document.getElementById('with-timestamps')?.checked;
        formData.append('with_timestamps', withTimestamps ? 'true' : 'false');

        const temperature = document.getElementById('temperature')?.value;
        if (temperature) formData.append('temperature', temperature);

        const noSpeechThreshold = document.getElementById('no-speech-threshold')?.value;
        if (noSpeechThreshold) formData.append('no_speech_threshold', noSpeechThreshold);

        this.showProgress('アップロード中...');

        try {
            const response = await fetch('/api/upload', {
                method: 'POST',
                body: formData,
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
            this.isUploading = false;
            this.updateTranscribeButtonState();
            await this.loadStats();
        }
    }

    updateTranscribeButtonState() {
        if (!this.transcribeButton) {
            return;
        }

        const disabled = !this.currentFile || this.isUploading;
        this.transcribeButton.disabled = disabled;

        const label = this.transcribeButtonLabel || '文字起こしを開始';
        const loadingLabel = this.transcribeButtonLoadingLabel || label;
        this.transcribeButton.textContent = this.isUploading ? loadingLabel : label;
    }

    displayResults(data, withTimestamps) {
        document.getElementById('result-text').textContent = data.text;
        document.getElementById('processing-time').textContent =
            data.processing_time ? data.processing_time.toFixed(2) : 'N/A';

        const durationValue = data.duration ?? (this.audioPlayer?.duration ?? null);
        document.getElementById('audio-duration').textContent =
            durationValue ? durationValue.toFixed(2) : 'N/A';

        const detectedLanguage = data.language
            || (this.languageSelect?.value ? this.languageSelect.value : '不明');
        document.getElementById('detected-language').textContent = detectedLanguage;

        const segments = withTimestamps && Array.isArray(data.segments) ? data.segments : [];
        this.currentSegments = segments;
        this.displaySegments(segments);

        const effectiveDuration = durationValue || 0;
        this.timelineData = {
            segments,
            duration: effectiveDuration,
        };
        this.buildTimeline(segments, effectiveDuration);
        this.updateAudioAvailability();

        this.resultsSection.style.display = 'block';
        this.currentResultData = data;
    }

    displaySegments(segments) {
        const wrapper = document.getElementById('segments-container');
        const container = document.getElementById('segments');
        if (!container || !wrapper) return;

        container.innerHTML = '';
        this.segmentElements = [];

        if (!segments.length) {
            wrapper.style.display = 'none';
            return;
        }

        wrapper.style.display = 'block';

        segments.forEach((segment, index) => {
            const segmentEl = document.createElement('div');
            segmentEl.className = 'segment';
            segmentEl.dataset.start = segment.start;
            segmentEl.dataset.end = segment.end;

            const timeEl = document.createElement('div');
            timeEl.className = 'segment-time';
            timeEl.textContent = `${this.formatTime(segment.start)} - ${this.formatTime(segment.end)}`;

            const textEl = document.createElement('div');
            textEl.className = 'segment-text';
            textEl.textContent = segment.text;

            segmentEl.appendChild(timeEl);
            segmentEl.appendChild(textEl);
            segmentEl.addEventListener('click', (event) => {
                event.stopPropagation();
                this.seekTo(segment.start);
                this.highlightSegment(index);
            });

            container.appendChild(segmentEl);
            this.segmentElements.push(segmentEl);
        });
    }

    buildTimeline(segments, duration) {
        if (!this.timelineContainer || !this.timelineSegmentsContainer) {
            return;
        }

        this.timelineSegmentsContainer.innerHTML = '';
        const totalDuration = duration || this.audioPlayer?.duration || 0;

        if (!totalDuration) {
            this.timelineProgress && (this.timelineProgress.style.width = '0%');
            return;
        }

        const safeSegments = segments || [];
        safeSegments.forEach((segment) => {
            if (typeof segment.start !== 'number' || typeof segment.end !== 'number') {
                return;
            }
            const segmentDuration = Math.max(segment.end - segment.start, 0);
            const widthPercent = Math.max((segmentDuration / totalDuration) * 100, 0.5);
            const leftPercent = Math.min((segment.start / totalDuration) * 100, 100);

            const timelineSegment = document.createElement('div');
            timelineSegment.className = 'timeline-segment';
            timelineSegment.style.left = `${leftPercent}%`;
            timelineSegment.style.width = `${Math.min(widthPercent, 100 - leftPercent)}%`;
            timelineSegment.title = `${this.formatTime(segment.start)} - ${this.formatTime(segment.end)}`;
            timelineSegment.addEventListener('click', (event) => {
                event.stopPropagation();
                this.seekTo(segment.start);
            });
            this.timelineSegmentsContainer.appendChild(timelineSegment);
        });

        this.updateTimelineProgress(0);
    }

    updateAudioAvailability() {
        if (!this.audioContainer) return;
        const hasAudio = Boolean(this.audioPlayer && this.audioPlayer.src);
        this.audioContainer.style.display = hasAudio ? 'block' : 'none';
    }

    handleTimelineClick(event) {
        if (!this.audioPlayer || (!this.audioPlayer.duration && !this.timelineData.duration)) {
            return;
        }

        const rect = this.timelineContainer.getBoundingClientRect();
        if (!rect.width) {
            return;
        }

        const ratio = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
        const duration = this.audioPlayer.duration || this.timelineData.duration;
        const targetTime = duration * ratio;
        this.seekTo(targetTime);
    }

    handleTimeUpdate() {
        if (!this.audioPlayer) return;
        const now = performance.now();
        if (now - this.lastTimelineUpdate < this.timelineUpdateInterval) {
            return;
        }
        this.lastTimelineUpdate = now;
        this.updateTimelineProgress(this.audioPlayer.currentTime);
        this.updateActiveSegment(this.audioPlayer.currentTime);
    }

    handleLoadedMetadata() {
        if (!this.audioPlayer) return;
        const duration = this.audioPlayer.duration || this.timelineData.duration;
        this.timelineData = {
            segments: this.currentSegments,
            duration,
        };
        this.buildTimeline(this.currentSegments, duration);
        this.updateTimelineProgress(this.audioPlayer.currentTime || 0);
    }

    updateTimelineProgress(currentTime) {
        if (!this.timelineProgress) return;
        const duration = this.audioPlayer?.duration || this.timelineData.duration;
        if (!duration) {
            this.timelineProgress.style.width = '0%';
            return;
        }
        const percentage = Math.min((currentTime / duration) * 100, 100);
        this.timelineProgress.style.width = `${percentage}%`;
    }

    updateActiveSegment(currentTime) {
        if (!this.currentSegments.length) return;

        let activeIndex = this.currentSegments.findIndex(
            (segment) => currentTime >= segment.start && currentTime < segment.end,
        );

        if (activeIndex === -1 && currentTime >= this.currentSegments[this.currentSegments.length - 1].end) {
            activeIndex = this.currentSegments.length - 1;
        }

        this.highlightSegment(activeIndex);
    }

    highlightSegment(index) {
        this.segmentElements.forEach((element, idx) => {
            if (idx === index) {
                element.classList.add('active');
            } else {
                element.classList.remove('active');
            }
        });
    }

    clearActiveSegment() {
        this.highlightSegment(-1);
        const endTime = this.audioPlayer?.duration || this.timelineData.duration || 0;
        this.updateTimelineProgress(endTime);
    }

    seekTo(time) {
        if (!this.audioPlayer) return;
        const duration = this.audioPlayer.duration || this.timelineData.duration;
        if (duration) {
            const clamped = Math.min(Math.max(time, 0), duration);
            this.audioPlayer.currentTime = clamped;
        } else {
            this.audioPlayer.currentTime = Math.max(time, 0);
        }

        this.audioPlayer.play().catch(() => {
            /* 再生がブロックされた場合は無視 */
        });
    }

    formatTime(seconds) {
        const mins = Math.floor(seconds / 60);
        const secs = Math.floor(seconds % 60);
        return `${mins}:${secs.toString().padStart(2, '0')}`;
    }

    showProgress(text) {
        const uploadContent = document.querySelector('.upload-content');
        if (uploadContent) {
            uploadContent.style.display = 'none';
        }
        if (this.uploadProgress) {
            this.uploadProgress.style.display = 'block';
        }
        if (this.progressText) {
            this.progressText.textContent = text;
        }
        if (this.progressFill) {
            this.progressFill.style.width = '100%';
        }
    }

    hideProgress() {
        const uploadContent = document.querySelector('.upload-content');
        if (uploadContent) {
            uploadContent.style.display = 'block';
        }
        if (this.uploadProgress) {
            this.uploadProgress.style.display = 'none';
        }
        if (this.progressFill) {
            this.progressFill.style.width = '0%';
        }
    }

    showNotification(message, type = 'info') {
        const notification = document.getElementById('notification');
        const notificationText = document.getElementById('notification-text');

        if (!notification || !notificationText) return;

        notificationText.textContent = message;
        notification.className = `notification ${type}`;
        notification.style.display = 'flex';

        window.clearTimeout(this.notificationTimeout);
        this.notificationTimeout = window.setTimeout(() => this.hideNotification(), 5000);
    }

    hideNotification() {
        const notification = document.getElementById('notification');
        if (notification) {
            notification.style.display = 'none';
        }
    }

    async copyText() {
        const text = document.getElementById('result-text')?.textContent || '';
        try {
            await navigator.clipboard.writeText(text);
            this.showNotification('テキストをクリップボードにコピーしました', 'success');
        } catch (error) {
            this.showNotification('コピーに失敗しました', 'error');
        }
    }

    downloadText() {
        const text = document.getElementById('result-text')?.textContent || '';
        const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
        this.downloadBlob(blob, 'transcription.txt');
    }

    downloadJSON() {
        if (!this.currentResultData) return;

        const blob = new Blob([JSON.stringify(this.currentResultData, null, 2)], {
            type: 'application/json;charset=utf-8',
        });
        this.downloadBlob(blob, 'transcription.json');
    }

    downloadBlob(blob, filename) {
        const url = URL.createObjectURL(blob);
        const anchor = document.createElement('a');
        anchor.href = url;
        anchor.download = filename;
        document.body.appendChild(anchor);
        anchor.click();
        document.body.removeChild(anchor);
        URL.revokeObjectURL(url);
    }

    clearResults() {
        if (this.resultsSection) {
            this.resultsSection.style.display = 'none';
        }
        this.currentResultData = null;
        this.currentFile = null;
        this.setUploadState(null);
        this.fileInput && (this.fileInput.value = '');
        this.segmentElements = [];
        this.currentSegments = [];
        this.timelineData = { segments: [], duration: 0 };

        if (this.audioPlayer) {
            this.audioPlayer.pause();
            this.audioPlayer.currentTime = 0;
            if (this.audioUrl) {
                URL.revokeObjectURL(this.audioUrl);
                this.audioUrl = null;
            }
            this.audioPlayer.removeAttribute('src');
            this.audioPlayer.load();
        }

        if (this.audioContainer) {
            this.audioContainer.style.display = 'none';
        }

        if (this.timelineSegmentsContainer) {
            this.timelineSegmentsContainer.innerHTML = '';
        }

        if (this.timelineProgress) {
            this.timelineProgress.style.width = '0%';
        }

        this.updateTranscribeButtonState();
    }
}

document.addEventListener('DOMContentLoaded', () => {
    new WhisperWebUI();
});
