import { buildResultTone, formatBytes, formatDimensions, pluralize } from './formatters.js';

export function renderApp(state, elements) {
    renderControls(state, elements);
    renderStatus(state, elements);
    renderPreview(state, elements);
}

function renderControls(state, elements) {
    elements.dropzone.classList.toggle('is-active', state.dragActive);
    elements.dropzone.disabled = state.isProcessing || state.isImporting;
    elements.dropzoneTitle.textContent = state.images.length ? 'Drop more images here' : 'Drop images here';

    for (const option of elements.formatOptions) {
        const isActive = option.dataset.format === state.format;
        option.classList.toggle('is-active', isActive);
        option.setAttribute('aria-checked', isActive ? 'true' : 'false');
        option.disabled = state.isProcessing;
    }

    elements.widthInput.value = state.width;
    elements.heightInput.value = state.height;
    elements.widthInput.disabled = state.isProcessing;
    elements.heightInput.disabled = state.isProcessing;
    elements.widthInput.classList.toggle('is-reference-value', state.resizeMode === 'none');
    elements.heightInput.classList.toggle('is-reference-value', state.resizeMode === 'none');
    elements.resizeReference.textContent = buildResizeReferenceText(state);
    elements.resizeHelper.textContent = state.validationMessage || buildResizeHelperText(state);
    elements.resizeHelper.classList.toggle('is-error', Boolean(state.validationMessage));

    elements.qualitySlider.value = String(state.quality);
    elements.qualitySlider.disabled = state.isProcessing || state.format === 'png';
    elements.qualityValue.textContent = state.format === 'png' ? 'Lossless' : String(state.quality);
    elements.qualityHelper.textContent =
        state.format === 'png'
            ? 'PNG is lossless. Quality disabled.'
            : 'JPEG, WEBP and AVIF respect this setting. PNG stays lossless.';

    elements.prefixInput.value = state.filenameComponent;
    elements.prefixInput.disabled = state.isProcessing;
    elements.prefixInput.placeholder = state.filenameMode === 'prefix' ? 'Enter prefix' : 'Enter postfix';

    if (elements.filenameToggle) {
        const toggleButtons = elements.filenameToggle.querySelectorAll('.toggle-button');
        for (const button of toggleButtons) {
            const isActive = button.dataset.mode === state.filenameMode;
            button.setAttribute('aria-pressed', isActive ? 'true' : 'false');
            button.disabled = state.isProcessing;
        }
    }

    elements.outputPath.textContent = state.outputDirectory || 'Loading your default Downloads folder...';
    elements.outputPath.title = state.outputDirectory;
    elements.chooseFolderButton.disabled = state.isProcessing;

    const hasImages = state.images.length > 0;
    elements.addImagesButton.disabled = state.isProcessing;
    elements.removeAllButton.disabled = state.isProcessing || !hasImages;
    elements.previewMeta.textContent = hasImages
        ? `${pluralize('image', state.images.length)} loaded`
        : 'No images added yet';

    elements.convertButton.disabled = state.isProcessing || !hasImages || Boolean(state.validationMessage);
    elements.convertButton.textContent = state.isProcessing ? 'Converting images...' : 'Bulk Convert';

    elements.statusSpinner.classList.toggle('is-visible', state.isImporting || state.isProcessing);
}

function renderStatus(state, elements) {
    elements.statusText.textContent = state.status.text;
    elements.statusText.dataset.kind = state.status.kind;
}

function renderPreview(state, elements) {
    if (!state.images.length) {
        elements.previewList.replaceChildren(buildEmptyState());
        return;
    }

    elements.previewList.replaceChildren(...state.images.map(buildPreviewCard));
}

function buildEmptyState() {
    const emptyState = document.createElement('div');
    emptyState.className = 'empty-state';

    const title = document.createElement('p');
    title.className = 'empty-title';
    title.textContent = 'No images added yet';

    const copy = document.createElement('p');
    copy.className = 'empty-copy';
    copy.textContent = 'Drop JPG, PNG, or WEBP files into the upload area to start.';

    emptyState.append(title, copy);
    return emptyState;
}

function buildPreviewCard(image) {
    const result = image.result;
    const displayFileType = buildDisplayFileType(image, result);
    const displayDimensions = buildDisplayDimensions(image, result);

    const card = document.createElement('article');
    card.className = 'preview-card';

    const thumbWrap = document.createElement('div');
    thumbWrap.className = 'thumb-wrap';

    const previewImage = document.createElement('img');
    previewImage.src = image.previewDataUrl;
    previewImage.alt = `${image.name} preview`;

    const removeButton = document.createElement('button');
    removeButton.className = 'thumb-remove-button remove-image-button';
    removeButton.type = 'button';
    removeButton.dataset.imageId = image.id;
    removeButton.setAttribute('aria-label', `Remove ${image.name}`);

    const removeIcon = document.createElement('span');
    removeIcon.setAttribute('aria-hidden', 'true');
    removeIcon.textContent = '×';
    removeButton.append(removeIcon);

    thumbWrap.append(previewImage, removeButton);

    const body = document.createElement('div');
    body.className = 'preview-body';

    const titleRow = document.createElement('div');
    titleRow.className = 'preview-title-row';

    const titleContent = document.createElement('div');

    const name = document.createElement('h3');
    name.className = 'preview-name';
    name.title = image.name;
    name.textContent = image.name;

    const subtitle = document.createElement('p');
    subtitle.className = 'preview-subtitle';
    subtitle.textContent = `${displayFileType} · ${displayDimensions}`;

    titleContent.append(name, subtitle);
    titleRow.append(titleContent);
    body.append(titleRow);

    if (result) {
        body.append(buildPreviewResult(result));
    }

    card.append(thumbWrap, body);
    return card;
}

function buildPreviewResult(result) {
    const resultElement = document.createElement('div');
    resultElement.className = `preview-result tone-${buildResultTone(result)}`;

    const chip = document.createElement('span');
    chip.className = 'result-chip';
    chip.textContent = result.success ? buildResultChipText(result) : result.message;

    resultElement.append(chip);
    return resultElement;
}

function buildResizeReferenceText(state) {
    if (!state.resizeReference) {
        return 'Waiting for images';
    }

    const base = formatDimensions(state.resizeReference.width, state.resizeReference.height);
    return state.resizeReference.mixedSizes ? `${base} from first image` : `${base} reference`;
}

function buildResizeHelperText(state) {
    if (!state.images.length) {
        return 'Aspect ratio is preserved automatically';
    }

    if (state.resizeMode === 'width') {
        return 'Height is calculated automatically for every image.';
    }

    if (state.resizeMode === 'height') {
        return 'Width is calculated automatically for every image.';
    }

    return state.resizeReference?.mixedSizes
        ? 'Original values are loaded from the first image. Edit either width or height to resize.'
        : 'Original values are loaded. Edit either width or height to resize.';
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

    const extension = outputName.split('.').pop()?.toLowerCase();
    switch (extension) {
        case 'jpg':
        case 'jpeg':
            return 'JPEG';
        case 'png':
            return 'PNG';
        case 'webp':
            return 'WEBP';
        case 'avif':
            return 'AVIF';
        default:
            return image.fileType;
    }
}

function buildResultChipText(result) {
    if (!result?.success) {
        return result?.message ?? '';
    }

    const outputSize = formatBytes(result.convertedSize);
    return `${outputSize} · ${result.message}`;
}
