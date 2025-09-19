async function waitForTauriApi(timeoutMs = 5000) {
  if (window.__TAURI__) {
    return window.__TAURI__;
  }

  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error("Tauri API の初期化に失敗しました"));
    }, timeoutMs);

    window.addEventListener(
      "tauri://ready",
      () => {
        clearTimeout(timer);
        if (window.__TAURI__) {
          resolve(window.__TAURI__);
        } else {
          reject(new Error("Tauri API が利用できません"));
        }
      },
      { once: true }
    );
  });
}

function initializeApp(tauriApi) {
  const statusEl = document.getElementById("status");

  if (
    !tauriApi ||
    !tauriApi.dialog ||
    !tauriApi.event ||
    !tauriApi.fs ||
    !tauriApi.tauri
  ) {
    const message = "必要な Tauri API が利用できません";
    console.error(message, tauriApi);
    if (statusEl) {
      statusEl.textContent = message;
      statusEl.dataset.state = "error";
    }
    return;
  }

  const { dialog, event, fs, tauri } = tauriApi;
  const { open, save } = dialog;
  const { listen } = event;
  const { invoke } = tauri;
  const { writeTextFile } = fs;

  if (!open || !save || !listen || !invoke || !writeTextFile) {
    const message = "必要な Tauri API が許可されていません";
    console.error(message);
    if (statusEl) {
      statusEl.textContent = message;
      statusEl.dataset.state = "error";
    }
    return;
  }

  const modelSelect = document.getElementById("model-select");
  const selectButton = document.getElementById("select-file");
  const startButton = document.getElementById("start-transcription");
  const copyButton = document.getElementById("copy-result");
  const saveButton = document.getElementById("save-result");
  const selectedFileLabel = document.getElementById("selected-file");
  const resultArea = document.getElementById("transcription-result");

  if (
    !selectButton ||
    !startButton ||
    !copyButton ||
    !saveButton ||
    !selectedFileLabel ||
    !statusEl ||
    !resultArea
  ) {
    console.error("必要な UI 要素が見つかりません");
    return;
  }

  let selectedFilePath = null;
  let isTranscribing = false;
  let isDownloadingModel = false;
  let isLoadingModels = false;
  let availableModels = [];

  function setStatus(message, state = "idle") {
    statusEl.textContent = message;
    statusEl.dataset.state = state;
  }

  function updateButtons() {
    const busy = isTranscribing || isDownloadingModel;
    startButton.disabled = busy || !selectedFilePath;
    selectButton.disabled = busy;
    if (modelSelect) {
      modelSelect.disabled = busy || isLoadingModels;
    }
    const hasResult = !!resultArea.value.trim();
    copyButton.disabled = !hasResult || busy;
    saveButton.disabled = !hasResult || busy;
  }

  function getModelLabel(modelId) {
    const entry = availableModels.find((model) => model.id === modelId);
    return entry ? entry.label : modelId;
  }

  function renderModelOptions(models) {
    if (!modelSelect) {
      return;
    }

    modelSelect.innerHTML = "";

    models.forEach((model) => {
      const option = document.createElement("option");
      option.value = model.id;
      option.textContent = `${model.label}${model.downloaded ? "" : " (未ダウンロード)"}`;
      option.dataset.path = model.path;
      modelSelect.appendChild(option);
      if (model.current) {
        modelSelect.value = model.id;
      }
    });

    if (!modelSelect.value && models.length > 0) {
      modelSelect.value = models[0].id;
    }
  }

  async function refreshModels() {
    if (!modelSelect) {
      return;
    }

    try {
      isLoadingModels = true;
      updateButtons();
      const models = await invoke("list_models");
      availableModels = Array.isArray(models) ? models : [];
      renderModelOptions(availableModels);
    } catch (error) {
      console.error("モデル一覧取得エラー", error);
      setStatus(`モデル一覧の取得に失敗しました: ${error}`, "error");
    } finally {
      isLoadingModels = false;
      updateButtons();
    }
  }

  if (modelSelect) {
    modelSelect.addEventListener("change", async (event) => {
      const modelId = event.target.value;
      if (!modelId) {
        return;
      }

      const previousId = availableModels.find((model) => model.current)?.id ?? null;
      if (previousId === modelId) {
        return;
      }

      try {
        isDownloadingModel = true;
        updateButtons();
        const label = getModelLabel(modelId);
        setStatus(`${label} を準備しています`, "loading");
        await invoke("select_model", { model_id: modelId });
      } catch (error) {
        console.error("モデル切り替えエラー", error);
        setStatus(`モデルの切り替えに失敗しました: ${error}`, "error");
        if (previousId) {
          modelSelect.value = previousId;
        }
      } finally {
        isDownloadingModel = false;
        updateButtons();
      }
    });
  }

  listen("model-download", ({ payload }) => {
    const status = payload?.status ?? "progress";

    if (status === "started" || status === "progress") {
      isDownloadingModel = true;
    }

    if (status === "error") {
      isDownloadingModel = false;
    }

    if (payload?.message) {
      const state = status === "error" ? "error" : status === "completed" ? "success" : "loading";
      setStatus(payload.message, state);
    }

    updateButtons();
  }).catch((error) => console.error("model-download listener error", error));

  listen("model-selected", async ({ payload }) => {
    const modelId = payload?.model_id;
    if (modelId && modelSelect) {
      modelSelect.value = modelId;
    }

    isDownloadingModel = false;
    updateButtons();

    await refreshModels();

    if (modelId) {
      const label = getModelLabel(modelId);
      setStatus(`${label} を選択しました`, "success");
    } else {
      setStatus("モデルを切り替えました", "success");
    }
  }).catch((error) => console.error("model-selected listener error", error));

  function updateSelectedFile(path) {
    selectedFilePath = path;
    if (path) {
      const name = path.split(/[\\/]/).pop();
      selectedFileLabel.textContent = name ?? path;
      selectedFileLabel.title = path;
    } else {
      selectedFileLabel.textContent = "未選択";
      selectedFileLabel.title = "";
    }
    updateButtons();
  }

  selectButton.addEventListener("click", async () => {
    try {
      const selection = await open({
        multiple: false,
        filters: [
          { name: "Audio", extensions: ["wav", "mp3", "flac", "m4a", "ogg"] }
        ]
      });

      if (!selection) {
        return;
      }

      const picked = Array.isArray(selection) ? selection[0] : selection;
      updateSelectedFile(picked);
      setStatus("文字起こしを開始できます", "idle");
    } catch (error) {
      console.error("ファイル選択ダイアログの起動に失敗しました", error);
      setStatus(`ファイル選択エラー: ${error}`, "error");
    }
  });

  startButton.addEventListener("click", async () => {
    if (!selectedFilePath || isTranscribing) {
      return;
    }

    try {
      isTranscribing = true;
      resultArea.value = "";
      setStatus("文字起こしを開始します", "loading");
      updateButtons();
      await invoke("transcribe_audio", { path: selectedFilePath });
    } catch (error) {
      console.error(error);
      setStatus(`コマンド実行エラー: ${error}`, "error");
      isTranscribing = false;
      updateButtons();
    }
  });

  copyButton.addEventListener("click", async () => {
    const text = resultArea.value;
    if (!text.trim()) {
      return;
    }
    try {
      if (!navigator.clipboard) {
        throw new Error("clipboard API が利用できません");
      }
      await navigator.clipboard.writeText(text);
      setStatus("結果をクリップボードにコピーしました", "success");
    } catch (error) {
      console.error(error);
      setStatus("クリップボードへコピーできませんでした", "error");
    }
  });

  saveButton.addEventListener("click", async () => {
    const text = resultArea.value;
    if (!text.trim()) {
      return;
    }

    const suggestedName = (() => {
      if (!selectedFilePath) return "transcript.txt";
      const name = selectedFilePath.split(/[\\/]/).pop() || "transcript";
      return `${name.replace(/\.[^.]+$/, "")}.txt`;
    })();

    const targetPath = await save({
      defaultPath: suggestedName,
      filters: [{ name: "Text", extensions: ["txt"] }]
    });

    if (!targetPath) {
      return;
    }

    try {
      await writeTextFile({ path: targetPath, contents: text });
      setStatus("ファイルを保存しました", "success");
    } catch (error) {
      console.error(error);
      setStatus("ファイルの保存に失敗しました", "error");
    }
  });

  listen("transcription-started", ({ payload }) => {
    isTranscribing = true;
    resultArea.value = "";
    setStatus(payload?.message ?? "文字起こしを開始します", "loading");
    updateButtons();
  }).catch((error) => console.error("transcription-started listener error", error));

  listen("transcription-progress", ({ payload }) => {
    if (!isTranscribing) {
      return;
    }
    setStatus(payload?.message ?? "処理中…", "loading");
  }).catch((error) => console.error("transcription-progress listener error", error));

  listen("transcription-completed", ({ payload }) => {
    isTranscribing = false;
    resultArea.value = payload?.text ?? "";
    const sourcePath = payload?.source_path ?? selectedFilePath;
    const name = sourcePath ? sourcePath.split(/[\\/]/).pop() : null;
    const message = name ? `文字起こしが完了しました (${name})` : "文字起こしが完了しました";
    setStatus(message, "success");
    updateButtons();
  }).catch((error) => console.error("transcription-completed listener error", error));

  listen("transcription-error", ({ payload }) => {
    isTranscribing = false;
    const message = payload?.message ?? "処理中にエラーが発生しました";
    setStatus(message, "error");
    updateButtons();
  }).catch((error) => console.error("transcription-error listener error", error));

  refreshModels().catch((error) => console.error("初期モデル読み込みエラー", error));
  updateButtons();
}

waitForTauriApi()
  .then((tauriApi) => {
    initializeApp(tauriApi);
  })
  .catch((error) => {
    console.error("アプリ初期化に失敗しました", error);
    const statusEl = document.getElementById("status");
    if (statusEl) {
      const message = error instanceof Error ? error.message : String(error);
      statusEl.textContent = `アプリ初期化に失敗しました: ${message}`;
      statusEl.dataset.state = "error";
    }
  });
