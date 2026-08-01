import { buildBrandTitle, buildResultTone, formatBytes, formatDimensions, pluralize } from './formatters.js';

export function renderApp(state, elements) {
    renderBrand(state, elements);
    renderView(state, elements);
    renderControls(state, elements);
    renderPresetPicker(state, elements);
    renderPresetForm(state, elements);
    renderPresetList(state, elements);
    renderMagicDirectoryForm(state, elements);
    renderMagicDirectoryList(state, elements);
    renderStatus(state, elements);
    renderPreview(state, elements);
}

function renderBrand(state, elements) {
    elements.brandTitle.textContent = buildBrandTitle(state.magicDirectoryChangeDetected);
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
    elements.resizeReference.disabled = state.isProcessing || !state.resizeReference;
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
    elements.showOutputFolderButton.disabled = !state.outputDirectory;

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

function renderView(state, elements) {
    const isConvertView = state.view === 'convert';

    elements.convertView.hidden = !isConvertView;
    elements.presetsView.hidden = state.view !== 'presets';
    elements.magicView.hidden = state.view !== 'magic';
    elements.convertActionBar.hidden = !isConvertView;

    for (const button of elements.appModeButtons) {
        const isActive = button.dataset.view === state.view;
        button.classList.toggle('is-active', isActive);
        button.setAttribute('aria-pressed', isActive ? 'true' : 'false');
    }
}

function renderMagicDirectoryForm(state, elements) {
    const form = state.magicDirectoryForm;
    const isSaving = state.isMagicDirectorySaving;

    elements.magicFormTitle.textContent = form.id ? 'Edit Magic Directory' : 'Add Magic Directory';
    elements.magicDirectoryPath.textContent = form.path || 'Choose a directory to watch...';
    elements.magicDirectoryPath.title = form.path;
    elements.magicChooseDirectoryButton.disabled = isSaving;
    elements.magicResetButton.disabled = isSaving;

    for (const option of elements.magicFormatOptions) {
        const isActive = form.formats.includes(option.dataset.format);
        option.classList.toggle('is-active', isActive);
        option.setAttribute('aria-pressed', isActive ? 'true' : 'false');
        option.disabled = isSaving;
    }

    if (!state.presets.length) {
        elements.magicPresetOptions.replaceChildren(
            buildPresetEmptyState('Create a preset before adding a magic directory.'),
        );
    } else {
        elements.magicPresetOptions.replaceChildren(
            ...state.presets.map(preset =>
                buildMagicPresetOption(preset, form.presetIds.includes(preset.id), isSaving),
            ),
        );
    }

    elements.magicEnabledButton.classList.toggle('is-active', form.enabled);
    elements.magicEnabledButton.setAttribute('aria-pressed', form.enabled ? 'true' : 'false');
    elements.magicEnabledButton.textContent = form.enabled ? 'Enabled' : 'Disabled';
    elements.magicEnabledButton.disabled = isSaving;
    elements.magicSaveButton.disabled = isSaving || !state.presets.length;
    elements.magicSaveButton.textContent = isSaving
        ? 'Saving Magic Directory...'
        : form.id
          ? 'Update Magic Directory'
          : 'Save Magic Directory';
}

function renderMagicDirectoryList(state, elements) {
    elements.magicCount.textContent = String(state.magicDirectories.length);
    elements.magicActivity.textContent = state.magicActivity.text;
    elements.magicActivity.dataset.kind = state.magicActivity.kind;

    if (state.magicDirectoriesLoading) {
        elements.magicList.replaceChildren(buildPresetEmptyState('Loading magic directories...'));
        return;
    }
    if (!state.magicDirectories.length) {
        elements.magicList.replaceChildren(buildPresetEmptyState('No magic directories saved yet.'));
        return;
    }
    elements.magicList.replaceChildren(
        ...state.magicDirectories.map(directory => buildMagicDirectoryCard(directory, state.presets)),
    );
}

function renderPresetPicker(state, elements) {
    const selectedValue = buildPresetSelectValue(state);
    const options = [
        buildOption('custom', 'Custom'),
        buildOption('default', 'Default'),
        ...state.presets.map(preset => buildOption(String(preset.id), preset.name)),
    ];

    elements.presetSelect.replaceChildren(...options);
    elements.presetSelect.value = selectedValue;
    elements.presetSelect.disabled = state.isProcessing || state.presetsLoading;
}

function renderPresetForm(state, elements) {
    const form = state.presetForm;
    const isSaving = state.isPresetSaving;

    elements.presetFormTitle.textContent = form.id ? 'Edit Preset' : 'Create Preset';
    elements.presetNameInput.value = form.name;
    elements.presetNameInput.disabled = isSaving;

    for (const option of elements.presetFormatOptions) {
        const isActive = option.dataset.format === form.format;
        option.classList.toggle('is-active', isActive);
        option.setAttribute('aria-checked', isActive ? 'true' : 'false');
        option.disabled = isSaving;
    }

    for (const button of elements.presetResizeModeOptions) {
        const isActive = button.dataset.mode === form.resizeMode;
        button.setAttribute('aria-pressed', isActive ? 'true' : 'false');
        button.disabled = isSaving;
    }

    elements.presetWidthInput.value = form.width;
    elements.presetHeightInput.value = form.height;
    elements.presetWidthInput.disabled = isSaving || form.resizeMode !== 'width';
    elements.presetHeightInput.disabled = isSaving || form.resizeMode !== 'height';
    elements.presetWidthInput.placeholder = form.resizeMode === 'width' ? 'Width' : 'Auto';
    elements.presetHeightInput.placeholder = form.resizeMode === 'height' ? 'Height' : 'Auto';

    elements.presetQualitySlider.value = String(form.quality);
    elements.presetQualitySlider.disabled = isSaving || form.format === 'png';
    elements.presetQualityValue.textContent = form.format === 'png' ? 'Lossless' : String(form.quality);

    for (const button of elements.presetFilenameModeOptions) {
        const isActive = button.dataset.mode === form.filenameMode;
        button.setAttribute('aria-pressed', isActive ? 'true' : 'false');
        button.disabled = isSaving;
    }

    elements.presetFilenameInput.value = form.filenameComponent;
    elements.presetFilenameInput.disabled = isSaving;
    elements.presetFilenameInput.placeholder = form.filenameMode === 'prefix' ? 'Enter prefix' : 'Enter postfix';
    elements.presetOutputPath.textContent = form.outputDirectory || 'Choose an output folder...';
    elements.presetOutputPath.title = form.outputDirectory;
    elements.presetChooseFolderButton.disabled = isSaving;
    elements.presetResetButton.disabled = isSaving;
    elements.presetSaveButton.disabled = isSaving;
    elements.presetSaveButton.textContent = isSaving ? 'Saving Preset...' : form.id ? 'Update Preset' : 'Save Preset';
}

function renderPresetList(state, elements) {
    elements.presetCount.textContent = String(state.presets.length);

    if (state.presetsLoading) {
        elements.presetList.replaceChildren(buildPresetEmptyState('Loading presets...'));
        return;
    }

    if (!state.presets.length) {
        elements.presetList.replaceChildren(buildPresetEmptyState('No presets saved yet.'));
        return;
    }

    elements.presetList.replaceChildren(...state.presets.map(buildPresetCard));
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

function buildOption(value, label) {
    const option = document.createElement('option');
    option.value = value;
    option.textContent = label;
    return option;
}

function buildPresetSelectValue(state) {
    if (state.selectedPresetId === 'default') {
        return 'default';
    }

    if (state.presets.some(preset => String(preset.id) === String(state.selectedPresetId))) {
        return String(state.selectedPresetId);
    }

    return 'custom';
}

function buildPresetCard(preset) {
    const card = document.createElement('article');
    card.className = 'preset-card';

    const body = document.createElement('div');
    body.className = 'preset-card-body';

    const name = document.createElement('h4');
    name.textContent = preset.name;

    const summary = document.createElement('p');
    summary.textContent = `${preset.format.toUpperCase()} · ${buildPresetResolutionText(preset)} · Quality ${preset.quality}`;

    const filename = document.createElement('p');
    filename.textContent = buildPresetFilenameText(preset);

    const output = document.createElement('p');
    output.className = 'preset-card-path';
    output.title = preset.outputDirectory;
    output.textContent = preset.outputDirectory;

    body.append(name, summary, filename, output);

    const actions = document.createElement('div');
    actions.className = 'preset-card-actions';
    actions.append(
        buildPresetActionButton('apply', preset.id, 'Apply', 'primary'),
        buildPresetActionButton('edit', preset.id, 'Edit', 'secondary'),
        buildPresetActionButton('duplicate', preset.id, 'Duplicate', 'secondary'),
        buildPresetActionButton('delete', preset.id, 'Delete', 'secondary'),
    );

    card.append(body, actions);
    return card;
}

function buildPresetActionButton(action, presetId, label, tone) {
    const button = document.createElement('button');
    button.className = `button ${tone} compact preset-action-button`;
    button.type = 'button';
    button.dataset.action = action;
    button.dataset.presetId = String(presetId);
    button.textContent = label;
    return button;
}

function buildMagicPresetOption(preset, selected, disabled) {
    const label = document.createElement('label');
    label.className = 'magic-preset-option';

    const input = document.createElement('input');
    input.type = 'checkbox';
    input.dataset.presetId = String(preset.id);
    input.checked = selected;
    input.disabled = disabled;

    const copy = document.createElement('span');
    const name = document.createElement('strong');
    name.textContent = preset.name;
    const details = document.createElement('small');
    details.textContent = `${preset.format.toUpperCase()} · ${buildPresetResolutionText(preset)}`;
    copy.append(name, details);
    label.append(input, copy);
    return label;
}

function buildMagicDirectoryCard(directory, presets) {
    const card = document.createElement('article');
    card.className = 'preset-card magic-directory-card';

    const body = document.createElement('div');
    body.className = 'preset-card-body';
    const title = document.createElement('h4');
    title.textContent = directoryName(directory.path);
    const status = document.createElement('p');
    status.className = directory.enabled ? 'magic-status-enabled' : 'magic-status-disabled';
    status.textContent = directory.enabled ? 'Watching' : 'Disabled';
    const formats = document.createElement('p');
    formats.textContent = `Formats: ${directory.formats.map(format => format.toUpperCase()).join(', ')}`;
    const selectedPresetNames = directory.presetIds
        .map(id => presets.find(preset => preset.id === id)?.name)
        .filter(Boolean);
    const presetSummary = document.createElement('p');
    presetSummary.textContent = selectedPresetNames.length
        ? `Presets: ${selectedPresetNames.join(', ')}`
        : 'No presets selected';
    const path = document.createElement('p');
    path.className = 'preset-card-path';
    path.title = directory.path;
    path.textContent = directory.path;
    body.append(title, status, formats, presetSummary, path);

    const actions = document.createElement('div');
    actions.className = 'preset-card-actions';
    actions.append(
        buildMagicActionButton('edit', directory.id, 'Edit'),
        buildMagicActionButton('delete', directory.id, 'Delete'),
    );
    card.append(body, actions);
    return card;
}

function buildMagicActionButton(action, id, label) {
    const button = document.createElement('button');
    button.className = 'button secondary compact magic-action-button';
    button.type = 'button';
    button.dataset.action = action;
    button.dataset.magicDirectoryId = String(id);
    button.textContent = label;
    return button;
}

function directoryName(path) {
    return path.split(/[\\/]/).filter(Boolean).pop() || path;
}

function buildPresetEmptyState(message) {
    const emptyState = document.createElement('div');
    emptyState.className = 'empty-state preset-empty-state';

    const title = document.createElement('p');
    title.className = 'empty-title';
    title.textContent = message;

    emptyState.append(title);
    return emptyState;
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

function buildPresetResolutionText(preset) {
    if (preset.resizeMode === 'width' && preset.width) {
        return `Width ${preset.width}px`;
    }

    if (preset.resizeMode === 'height' && preset.height) {
        return `Height ${preset.height}px`;
    }

    return 'Original resolution';
}

function buildPresetFilenameText(preset) {
    const component = preset.filenameComponent?.trim();
    if (!component) {
        return 'No filename component';
    }

    const label = preset.filenameMode === 'postfix' ? 'Postfix' : 'Prefix';
    return `${label}: ${component}`;
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
