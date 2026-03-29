import {
  buildResultTone,
  escapeHtml,
  formatBytes,
  formatDimensions,
  pluralize,
} from "./formatters.js";

export function renderApp(state, elements) {
  renderControls(state, elements);
  renderStatus(state, elements);
  renderPreview(state, elements);
}

function renderControls(state, elements) {
  elements.dropzone.classList.toggle("is-active", state.dragActive);
  elements.dropzone.disabled = state.isProcessing || state.isImporting;
  elements.dropzoneTitle.textContent = state.images.length
    ? "Drop more images here"
    : "Drop images here";

  for (const option of elements.formatOptions) {
    const isActive = option.dataset.format === state.format;
    option.classList.toggle("is-active", isActive);
    option.setAttribute("aria-checked", isActive ? "true" : "false");
    option.disabled = state.isProcessing;
  }

  elements.widthInput.value = state.width;
  elements.heightInput.value = state.height;
  elements.widthInput.disabled = state.isProcessing;
  elements.heightInput.disabled = state.isProcessing;
  elements.widthInput.classList.toggle("is-reference-value", state.resizeMode === "none");
  elements.heightInput.classList.toggle("is-reference-value", state.resizeMode === "none");
  elements.resizeReference.textContent = buildResizeReferenceText(state);
  elements.resizeHelper.textContent =
    state.validationMessage || buildResizeHelperText(state);
  elements.resizeHelper.classList.toggle("is-error", Boolean(state.validationMessage));

  elements.qualitySlider.value = String(state.quality);
  elements.qualitySlider.disabled = state.isProcessing || state.format === "png";
  elements.qualityValue.textContent =
    state.format === "png" ? "Lossless" : String(state.quality);
  elements.qualityHelper.textContent =
    state.format === "png"
      ? "PNG is lossless. Quality disabled."
      : "JPEG and WEBP respect this setting. PNG stays lossless.";

  elements.prefixInput.value = state.prefix;
  elements.prefixInput.disabled = state.isProcessing;

  elements.outputPath.textContent =
    state.outputDirectory || "Loading your default Downloads folder...";
  elements.outputPath.title = state.outputDirectory;
  elements.chooseFolderButton.disabled = state.isProcessing;

  const hasImages = state.images.length > 0;
  elements.addImagesButton.disabled = state.isProcessing;
  elements.removeAllButton.disabled = state.isProcessing || !hasImages;
  elements.previewMeta.textContent = hasImages
    ? `${pluralize("image", state.images.length)} loaded`
    : "No images added yet";

  elements.convertButton.disabled =
    state.isProcessing || !hasImages || Boolean(state.validationMessage);
  elements.convertButton.textContent = state.isProcessing
    ? "Converting images..."
    : "Bulk Convert";

  elements.statusSpinner.classList.toggle(
    "is-visible",
    state.isImporting || state.isProcessing
  );
}

function renderStatus(state, elements) {
  elements.statusText.textContent = state.status.text;
  elements.statusText.dataset.kind = state.status.kind;
}

function renderPreview(state, elements) {
  if (!state.images.length) {
    elements.previewList.innerHTML = `
      <div class="empty-state">
        <p class="empty-title">No images added yet</p>
        <p class="empty-copy">Drop JPG, PNG, or WEBP files into the upload area to start.</p>
      </div>
    `;
    return;
  }

  elements.previewList.innerHTML = state.images
    .map((image) => {
      const result = image.result;
      const tone = buildResultTone(result);
      const displayFileType = buildDisplayFileType(image, result);
      const displayDimensions = buildDisplayDimensions(image, result);
      const resultMarkup = result
        ? result.success
          ? `
            <div class="preview-result tone-${tone}">
              <span class="result-chip">
                ${escapeHtml(buildResultChipText(result))}
              </span>
            </div>
          `
          : `
            <div class="preview-result tone-${tone}">
              <span class="result-chip">${escapeHtml(result.message)}</span>
            </div>
          `
        : "";

      return `
        <article class="preview-card">
          <div class="thumb-wrap">
            <img src="${image.previewDataUrl}" alt="${escapeHtml(image.name)} preview" />
            <button class="thumb-remove-button remove-image-button" type="button" data-image-id="${image.id}" aria-label="Remove ${escapeHtml(image.name)}">
              <span aria-hidden="true">×</span>
            </button>
          </div>
          <div class="preview-body">
            <div class="preview-title-row">
              <div>
                <h3 class="preview-name" title="${escapeHtml(image.name)}">${escapeHtml(image.name)}</h3>
                <p class="preview-subtitle">${displayFileType} · ${displayDimensions}</p>
              </div>
            </div>
            ${resultMarkup}
          </div>
        </article>
      `;
    })
    .join("");
}

function buildResizeReferenceText(state) {
  if (!state.resizeReference) {
    return "Waiting for images";
  }

  const base = formatDimensions(state.resizeReference.width, state.resizeReference.height);
  return state.resizeReference.mixedSizes
    ? `${base} from first image`
    : `${base} reference`;
}

function buildResizeHelperText(state) {
  if (!state.images.length) {
    return "Aspect ratio is preserved automatically";
  }

  if (state.resizeMode === "width") {
    return "Height is calculated automatically for every image.";
  }

  if (state.resizeMode === "height") {
    return "Width is calculated automatically for every image.";
  }

  return state.resizeReference?.mixedSizes
    ? "Original values are loaded from the first image. Edit either width or height to resize."
    : "Original values are loaded. Edit either width or height to resize.";
}

function buildDisplayDimensions(image, result) {
  if (result?.success && result.convertedWidth && result.convertedHeight) {
    return formatDimensions(result.convertedWidth, result.convertedHeight);
  }

  return formatDimensions(image.width, image.height);
}

function buildDisplayFileType(image, result) {
  const outputName = result?.success ? result.outputName : null;
  if (!outputName) {
    return image.fileType;
  }

  const extension = outputName.split(".").pop()?.toLowerCase();
  switch (extension) {
    case "jpg":
    case "jpeg":
      return "JPEG";
    case "png":
      return "PNG";
    case "webp":
      return "WEBP";
    default:
      return image.fileType;
  }
}

function buildResultChipText(result) {
  if (!result?.success) {
    return result?.message ?? "";
  }

  const outputSize = formatBytes(result.convertedSize);
  return `${outputSize} · ${result.message}`;
}
