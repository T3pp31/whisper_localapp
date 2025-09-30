/**
 * WebRTCリアルタイム文字起こしクライアント
 */
class RealtimeWebRTCClient {
    constructor(sessionId, iceServers, onTranscript) {
        this.sessionId = sessionId;
        this.iceServers = iceServers;
        this.onTranscript = onTranscript;

        this.peerConnection = null;
        this.dataChannel = null;
        this.websocket = null;
        this.localStream = null;
        this.isConnected = false;
    }

    /**
     * WebRTC接続を開始
     */
    async start() {
        try {
            // マイク音声取得
            this.localStream = await navigator.mediaDevices.getUserMedia({
                audio: {
                    echoCancellation: true,
                    noiseSuppression: true,
                    autoGainControl: true,
                    sampleRate: 48000,
                    channelCount: 1
                }
            });

            console.log('マイク音声取得成功');

            // WebSocket接続
            await this.connectWebSocket();

            // PeerConnection作成
            await this.createPeerConnection();

            // 音声トラック追加
            this.localStream.getTracks().forEach(track => {
                this.peerConnection.addTrack(track, this.localStream);
                console.log('音声トラック追加:', track.kind);
            });

            // Data Channel作成
            this.dataChannel = this.peerConnection.createDataChannel('transcripts', {
                ordered: true
            });

            this.setupDataChannel();

            // Offer作成
            const offer = await this.peerConnection.createOffer();
            await this.peerConnection.setLocalDescription(offer);

            console.log('Offer SDP作成完了');

            // WebSocket経由でOfferを送信
            this.sendSignalingMessage({
                type: 'offer',
                session_id: this.sessionId,
                sdp: offer.sdp
            });

            this.isConnected = true;
            console.log('WebRTC接続開始');

        } catch (error) {
            console.error('WebRTC接続エラー:', error);
            throw error;
        }
    }

    /**
     * WebSocket接続
     */
    async connectWebSocket() {
        return new Promise((resolve, reject) => {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            const wsUrl = `${protocol}//${window.location.host}/ws/realtime/${this.sessionId}`;

            this.websocket = new WebSocket(wsUrl);

            this.websocket.onopen = () => {
                console.log('WebSocket接続確立');
                resolve();
            };

            this.websocket.onmessage = (event) => {
                this.handleSignalingMessage(JSON.parse(event.data));
            };

            this.websocket.onerror = (error) => {
                console.error('WebSocketエラー:', error);
                reject(error);
            };

            this.websocket.onclose = () => {
                console.log('WebSocket切断');
            };
        });
    }

    /**
     * PeerConnection作成
     */
    async createPeerConnection() {
        const config = {
            iceServers: this.iceServers
        };

        this.peerConnection = new RTCPeerConnection(config);

        // ICE候補イベント
        this.peerConnection.onicecandidate = (event) => {
            if (event.candidate) {
                console.log('ICE Candidate:', event.candidate.candidate);
                this.sendSignalingMessage({
                    type: 'ice_candidate',
                    session_id: this.sessionId,
                    candidate: event.candidate.candidate
                });
            }
        };

        // 接続状態変化
        this.peerConnection.onconnectionstatechange = () => {
            console.log('接続状態:', this.peerConnection.connectionState);
        };

        // ICE接続状態変化
        this.peerConnection.oniceconnectionstatechange = () => {
            console.log('ICE接続状態:', this.peerConnection.iceConnectionState);
        };

        console.log('PeerConnection作成完了');
    }

    /**
     * Data Channel設定
     */
    setupDataChannel() {
        this.dataChannel.onopen = () => {
            console.log('Data Channel開通');
        };

        this.dataChannel.onmessage = (event) => {
            console.log('文字起こし受信:', event.data);

            try {
                const transcript = JSON.parse(event.data);
                if (this.onTranscript) {
                    this.onTranscript(transcript);
                }
            } catch (e) {
                console.error('文字起こしデータ解析エラー:', e);
            }
        };

        this.dataChannel.onerror = (error) => {
            console.error('Data Channelエラー:', error);
        };

        this.dataChannel.onclose = () => {
            console.log('Data Channel閉鎖');
        };
    }

    /**
     * シグナリングメッセージ送信
     */
    sendSignalingMessage(message) {
        if (this.websocket && this.websocket.readyState === WebSocket.OPEN) {
            this.websocket.send(JSON.stringify(message));
        } else {
            console.error('WebSocketが開いていません');
        }
    }

    /**
     * シグナリングメッセージ処理
     */
    async handleSignalingMessage(message) {
        console.log('シグナリングメッセージ受信:', message.type);

        switch (message.type) {
            case 'answer':
                await this.handleAnswer(message.sdp);
                break;

            case 'ice_candidate':
                await this.handleIceCandidate(message.candidate);
                break;

            case 'error':
                console.error('サーバーエラー:', message.message);
                break;

            default:
                console.log('未知のメッセージタイプ:', message.type);
        }
    }

    /**
     * Answer SDP処理
     */
    async handleAnswer(sdp) {
        try {
            const answer = new RTCSessionDescription({
                type: 'answer',
                sdp: sdp
            });

            await this.peerConnection.setRemoteDescription(answer);
            console.log('Answer SDP設定完了');
        } catch (error) {
            console.error('Answer処理エラー:', error);
        }
    }

    /**
     * ICE Candidate処理
     */
    async handleIceCandidate(candidate) {
        try {
            const iceCandidate = new RTCIceCandidate({
                candidate: candidate
            });

            await this.peerConnection.addIceCandidate(iceCandidate);
            console.log('ICE Candidate追加完了');
        } catch (error) {
            console.error('ICE Candidate処理エラー:', error);
        }
    }

    /**
     * 接続終了
     */
    stop() {
        // Data Channel閉鎖
        if (this.dataChannel) {
            this.dataChannel.close();
            this.dataChannel = null;
        }

        // PeerConnection閉鎖
        if (this.peerConnection) {
            this.peerConnection.close();
            this.peerConnection = null;
        }

        // WebSocket切断
        if (this.websocket) {
            this.websocket.close();
            this.websocket = null;
        }

        // 音声ストリーム停止
        if (this.localStream) {
            this.localStream.getTracks().forEach(track => track.stop());
            this.localStream = null;
        }

        this.isConnected = false;
        console.log('WebRTC接続終了');
    }
}

// グローバルに公開
window.RealtimeWebRTCClient = RealtimeWebRTCClient;