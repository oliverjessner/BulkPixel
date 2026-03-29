import { sanitizeNumberInput, pluralize } from "./formatters.js";
import { renderApp } from "./ui.js";

const { invoke } = window.__TAURI__.core;
const dialogApi = window.__TAURI__.dialog;
const webviewApi = window.__TAURI__.webview;

const state = {
  images: [],
  results: [],
  summary: null,
  format: "jpeg",
  width: "",
  height: "",
  resizeMode: "none",
  resizeReference: null,
  quality: 90,
  prefix: "",
  outputDirectory: "",
  isProcessing: false,
  isImporting: false,
  dragActive: false,
  validationMessage: "",
  status: {
    kind: "info",
    text: "Choose images to begin.",
  },
};

const elements = {};

window.addEventListener("DOMContentLoaded", async () => {
  cacheElements();
  bindEvents();
  render();
  await hydrateDefaultOutputDirectory();
  await bindNativeDragDrop();
});

function cacheElements() {
  elements.dropzone = document.querySelector("#dropzone");
  elements.dropzoneTitle = document.querySelector("#dropzone-title");
  elements.formatOptions = [...document.querySelectorAll(".format-option")];
  elements.widthInput = document.querySelector("#width-input");
  elements.heightInput = document.querySelector("#height-input");
  elements.resizeReference = document.querySelector("#resize-reference");
  elements.resizeHelper = document.querySelector("#resize-helper");
  elements.qualitySlider = document.querySelector("#quality-slider");
  elements.qualityValue = document.querySelector("#quality-value");
  elements.qualityHelper = document.querySelector("#quality-helper");
  elements.prefixInput = document.querySelector("#prefix-input");
  elements.outputPath = document.querySelector("#output-path");
  elements.chooseFolderButton = document.querySelector("#choose-folder-button");
  elements.addImagesButton = document.querySelector("#add-images-button");
  elements.removeAllButton = document.querySelector("#remove-all-button");
  elements.previewMeta = document.querySelector("#preview-meta");
  elements.previewList = document.querySelector("#preview-list");
  elements.statusSpinner = document.querySelector("#status-spinner");
  elements.statusText = document.querySelector("#status-text");
  elements.convertButton = document.querySelector("#convert-button");
}

function bindEvents() {
  elements.dropzone.addEventListener("click", () => {
    if (!state.isProcessing) {
      void pickImages();
    }
  });

  elements.addImagesButton.addEventListener("click", () => {
    void pickImages();
  });

  elements.removeAllButton.addEventListener("click", () => {
    state.images = [];
    syncResizeReference();
    clearResults();
    setStatus("info", "All images were removed.");
    render();
  });

  elements.formatOptions.forEach((option) => {
    option.addEventListener("click", () => {
      if (state.isProcessing) {
        return;
      }

      state.format = option.dataset.format;
      clearResults();
      validateState();
      render();
    });
  });

  elements.widthInput.addEventListener("input", (event) => {
    const nextValue = sanitizeNumberInput(event.target.value);

    if (!nextValue) {
      state.resizeMode = "none";
      syncResizeReference();
    } else {
      state.resizeMode = "width";
      state.width = nextValue;
      state.height = derivePairedDimension("width", nextValue);
    }

    clearResults();
    validateState();
    render();
  });

  elements.heightInput.addEventListener("input", (event) => {
    const nextValue = sanitizeNumberInput(event.target.value);

    if (!nextValue) {
      state.resizeMode = "none";
      syncResizeReference();
    } else {
      state.resizeMode = "height";
      state.height = nextValue;
      state.width = derivePairedDimension("height", nextValue);
    }

    clearResults();
    validateState();
    render();
  });

  elements.qualitySlider.addEventListener("input", (event) => {
    state.quality = Number(event.target.value);
    clearResults();
    render();
  });

  elements.prefixInput.addEventListener("input", (event) => {
    state.prefix = event.target.value;
    clearResults();
    render();
  });

  elements.chooseFolderButton.addEventListener("click", () => {
    void chooseOutputDirectory();
  });

  elements.previewList.addEventListener("click", (event) => {
    const button = event.target.closest(".remove-image-button");
    if (!button || state.isProcessing) {
      return;
    }

    state.images = state.images.filter((image) => image.id !== button.dataset.imageId);
    syncResizeReference();
    clearResults();
    setStatus("info", "Image removed.");
    render();
  });

  elements.convertButton.addEventListener("click", () => {
    void handleBulkConvert();
  });
}

async function hydrateDefaultOutputDirectory() {
  try {
    state.outputDirectory = await invoke("get_default_output_directory");
    render();
  } catch (error) {
    setStatus("error", normaliseError(error, "Unable to load the default output folder."));
    render();
  }
}

async function bindNativeDragDrop() {
  if (!webviewApi?.getCurrentWebview) {
    return;
  }

  try {
    const currentWebview = webviewApi.getCurrentWebview();
    await currentWebview.onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        state.dragActive = true;
        render();
        return;
      }

      if (event.payload.type === "drop") {
        state.dragActive = false;
        render();
        void addImagePaths(event.payload.paths ?? []);
        return;
      }

      state.dragActive = false;
      render();
    });
  } catch (error) {
    setStatus("warning", normaliseError(error, "Drag and drop is unavailable in this environment."));
    render();
  }
}

async function pickImages() {
  try {
    const selection = await dialogApi.open({
      directory: false,
      multiple: true,
      filters: [
        {
          name: "Images",
          extensions: ["jpg", "jpeg", "png", "webp"],
        },
      ],
    });
    const paths = normalizeDialogSelection(selection);

    if (!paths.length) {
      return;
    }

    await addImagePaths(paths);
  } catch (error) {
    setStatus("error", normaliseError(error, "Unable to open the image picker."));
    render();
  }
}

async function chooseOutputDirectory() {
  try {
    const selection = await dialogApi.open({
      directory: true,
      multiple: false,
      defaultPath: state.outputDirectory || undefined,
    });

    const [path] = normalizeDialogSelection(selection);
    if (!path) {
      return;
    }

    state.outputDirectory = path;
    clearResults();
    setStatus("info", "Output folder updated.");
    render();
  } catch (error) {
    setStatus("error", normaliseError(error, "Unable to choose an output folder."));
    render();
  }
}

async function addImagePaths(paths) {
  const uniquePaths = [...new Set(paths.filter(Boolean))];
  const existingPaths = new Set(state.images.map((image) => image.path));
  const freshPaths = uniquePaths.filter((path) => !existingPaths.has(path));

  if (!freshPaths.length) {
    setStatus("info", "Those images are already loaded.");
    render();
    return;
  }

  state.isImporting = true;
  state.status = {
    kind: "info",
    text: `Importing ${pluralize("image", freshPaths.length)}...`,
  };
  render();

  try {
    const response = await invoke("probe_images_command", { paths: freshPaths });
    const loadedImages = response.loaded.map((image) => ({
      id: createImageId(),
      ...image,
      result: null,
    }));

    state.images.push(...loadedImages);
    syncResizeReference();
    clearResults();

    if (response.rejected.length) {
      setStatus(
        "warning",
        `${pluralize("image", loadedImages.length)} added. ${pluralize("file", response.rejected.length)} skipped.`
      );
    } else {
      setStatus("success", `${pluralize("image", loadedImages.length)} added.`);
    }
  } catch (error) {
    setStatus("error", normaliseError(error, "Unable to inspect the selected images."));
  } finally {
    state.isImporting = false;
    validateState();
    render();
  }
}

async function handleBulkConvert() {
  const validationMessage = validateState();
  if (validationMessage) {
    setStatus("error", validationMessage);
    render();
    return;
  }

  const request = {
    images: state.images.map((image) => ({ path: image.path })),
    format: state.format,
    resize: {
      width: state.resizeMode === "width" && state.width ? Number(state.width) : null,
      height: state.resizeMode === "height" && state.height ? Number(state.height) : null,
    },
    quality: state.quality,
    prefix: state.prefix,
    outputDir: state.outputDirectory,
  };

  state.isProcessing = true;
  state.status = {
    kind: "info",
    text: `Converting ${pluralize("image", state.images.length)}...`,
  };
  render();

  try {
    const response = await invoke("bulk_convert_images", { request });
    const resultsByPath = new Map(response.results.map((result) => [result.inputPath, result]));

    state.results = response.results;
    state.summary = response.summary;
    state.images = state.images.map((image) => ({
      ...image,
      result: resultsByPath.get(image.path) ?? null,
    }));

    if (response.summary.failureCount === 0) {
      setStatus(
        "success",
        `${pluralize("image", response.summary.successCount)} converted successfully.`
      );
    } else if (response.summary.successCount > 0) {
      setStatus(
        "warning",
        `${pluralize("image", response.summary.successCount)} converted successfully. ${pluralize("image", response.summary.failureCount)} failed.`
      );
    } else {
      setStatus("error", "No images were converted. Review the results for details.");
    }
  } catch (error) {
    setStatus("error", normaliseError(error, "Bulk conversion failed."));
  } finally {
    state.isProcessing = false;
    validateState();
    render();
  }
}

function validateState() {
  let message = "";

  if (state.resizeMode === "width" && (!state.width || Number(state.width) <= 0)) {
    message = "Width must be greater than zero.";
  } else if (state.resizeMode === "height" && (!state.height || Number(state.height) <= 0)) {
    message = "Height must be greater than zero.";
  }

  state.validationMessage = message;
  return message;
}

function clearResults() {
  state.results = [];
  state.summary = null;
  state.images = state.images.map((image) => ({ ...image, result: null }));
}

function syncResizeReference() {
  if (!state.images.length) {
    state.resizeReference = null;
    state.resizeMode = "none";
    state.width = "";
    state.height = "";
    return;
  }

  const [firstImage] = state.images;
  const mixedSizes = state.images.some(
    (image) => image.width !== firstImage.width || image.height !== firstImage.height
  );

  state.resizeReference = {
    width: firstImage.width,
    height: firstImage.height,
    mixedSizes,
  };

  if (state.resizeMode === "none") {
    state.width = String(firstImage.width);
    state.height = String(firstImage.height);
  } else if (state.resizeMode === "width" && state.width) {
    state.height = derivePairedDimension("width", state.width);
  } else if (state.resizeMode === "height" && state.height) {
    state.width = derivePairedDimension("height", state.height);
  }
}

function derivePairedDimension(mode, rawValue) {
  if (!state.resizeReference) {
    return "";
  }

  const numericValue = Number(rawValue);
  if (!Number.isFinite(numericValue) || numericValue <= 0) {
    return "";
  }

  if (mode === "width") {
    const derived = Math.round(
      (numericValue * state.resizeReference.height) / state.resizeReference.width
    );
    return String(Math.max(1, derived));
  }

  const derived = Math.round(
    (numericValue * state.resizeReference.width) / state.resizeReference.height
  );
  return String(Math.max(1, derived));
}

function normalizeDialogSelection(selection) {
  if (!selection) {
    return [];
  }

  return Array.isArray(selection) ? selection : [selection];
}

function createImageId() {
  if (window.crypto?.randomUUID) {
    return window.crypto.randomUUID();
  }

  return `image-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function normaliseError(error, fallbackMessage) {
  if (typeof error === "string" && error.trim()) {
    return error;
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  return fallbackMessage;
}

function setStatus(kind, text) {
  state.status = { kind, text };
}

function render() {
  renderApp(state, elements);
}
