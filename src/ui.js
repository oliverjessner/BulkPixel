import {
    buildResultTone,
    buildSummaryDeltaText,
    formatBytes,
    formatDimensions,
    pluralize,
} from './formatters.js';

export function renderApp(state, elements) {
    renderBrand(elements);
    renderGlobalWatcherStatus(state, elements);
    renderView(state, elements);
    renderControls(state, elements);
    renderPresetPicker(state, elements);
    renderPresetForm(state, elements);
    renderPresetList(state, elements);
    renderMagicDirectoryForm(state, elements);
    renderMagicDirectoryList(state, elements);
    renderConversionActionBar(state, elements);
    renderPreview(state, elements);
}

function renderBrand(elements) {
    elements.brandTitle.textContent = 'BulkPixel';
}

function renderGlobalWatcherStatus(state, elements) {
    const watcherStatus = buildGlobalWatcherStatus(state);
    elements.globalWatcherStatus.dataset.kind = watcherStatus.kind;
    elements.globalWatcherStatus.title = watcherStatus.detail;
    elements.globalWatcherStatusLabel.textContent = watcherStatus.label;
}

export function buildGlobalWatcherStatus(state) {
    const directories = state.magicDirectories ?? [];
    const activeCount = directories.filter(directory => directory.enabled).length;
    const activity = state.magicActivity ?? { kind: 'info', text: '' };

    if (activity.kind === 'error') {
        return {
            kind: 'error',
            label: 'Watched folder error',
            detail: activity.text || 'Watched folder error.',
            activeCount,
        };
    }

    if (state.magicDirectoryChangeDetected) {
        return {
            kind: 'processing',
            label: activeCount ? `Processing · ${pluralize('folder', activeCount)}` : 'Processing',
            detail: activity.text || 'Processing files from watched folders.',
            activeCount,
        };
    }

    if (activeCount > 0) {
        return {
            kind: 'watching',
            label: `Watching ${pluralize('folder', activeCount)}`,
            detail: 'Watched folders are active while BulkPixel is open.',
            activeCount,
        };
    }

    if (directories.length > 0) {
        return {
            kind: 'idle',
            label: 'No active folders',
            detail: 'All configured watched folders are disabled.',
            activeCount,
        };
    }

    return {
        kind: 'idle',
        label: 'No watched folders',
        detail: 'No watched folders are configured.',
        activeCount,
    };
}

function renderControls(state, elements) {
    const hasImages = state.images.length > 0;
    elements.dropzone.hidden = hasImages;
    elements.imagesView.hidden = !hasImages;
    elements.loadedDropOverlay.hidden = !hasImages || !state.dragActive;
    elements.dropzone.classList.toggle('is-active', state.dragActive);
    elements.dropzone.disabled = state.isProcessing || state.isImporting;
    elements.dropzoneTitle.textContent = state.dragActive ? 'Drop images to add them' : 'Drop images here';

    for (const option of elements.formatOptions) {
        const isActive = option.dataset.format === state.format;
        option.classList.toggle('is-active', isActive);
        option.setAttribute('aria-checked', isActive ? 'true' : 'false');
        option.disabled = state.isProcessing;
    }

    for (const option of elements.resizeModeOptions) {
        const isActive = option.dataset.mode === state.resizeMode;
        option.setAttribute('aria-pressed', isActive ? 'true' : 'false');
        option.disabled = state.isProcessing;
    }

    const resizeInputs = buildResizeInputState(state);
    elements.widthInput.value = resizeInputs.widthValue;
    elements.heightInput.value = resizeInputs.heightValue;
    elements.widthInput.readOnly = resizeInputs.widthReadOnly;
    elements.heightInput.readOnly = resizeInputs.heightReadOnly;
    elements.widthInput.disabled = state.isProcessing;
    elements.heightInput.disabled = state.isProcessing;

    elements.qualitySlider.value = String(state.quality);
    elements.qualitySlider.disabled = state.isProcessing || state.format === 'png';
    elements.qualityValue.textContent = state.format === 'png' ? 'Lossless' : String(state.quality);
    elements.qualityHelper.textContent =
        state.format === 'png'
            ? 'PNG uses lossless compression and preserves 16-bit source channels. Quality disabled.'
            : 'JPEG, WEBP and AVIF respect this setting. PNG uses lossless compression.';

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

    elements.addImagesButton.disabled = state.isProcessing || state.isImporting;
    elements.previewMeta.textContent = hasImages ? buildImageLibraryMeta(state.images) : '';
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

    elements.magicFormTitle.textContent = form.id ? 'Edit Watched Folder' : 'Add Watched Folder';
    elements.magicNameInput.value = form.name;
    elements.magicNameInput.disabled = isSaving;
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
            buildPresetEmptyState('Create a preset before adding a watched folder.'),
        );
    } else {
        elements.magicPresetOptions.replaceChildren(
            ...state.presets.map(preset =>
                buildMagicPresetOption(preset, form.presetIds.includes(preset.id), isSaving),
            ),
        );
    }

    elements.magicEnabledButton.classList.toggle('is-active', form.enabled);
    elements.magicEnabledButton.setAttribute('aria-checked', form.enabled ? 'true' : 'false');
    elements.magicEnabledButton.disabled = isSaving;
    elements.magicSaveButton.disabled = isSaving || !state.presets.length;
    elements.magicSaveButton.textContent = isSaving
        ? 'Saving Watched Folder...'
        : form.id
          ? 'Update Watched Folder'
          : 'Save Watched Folder';
}

function renderMagicDirectoryList(state, elements) {
    elements.magicCount.textContent = String(state.magicDirectories.length);
    elements.magicActivity.textContent = state.magicActivity.text;
    elements.magicActivity.dataset.kind = state.magicActivity.kind;

    if (state.magicDirectoriesLoading) {
        elements.magicList.replaceChildren(buildPresetEmptyState('Loading watched folders...'));
        return;
    }
    if (!state.magicDirectories.length) {
        elements.magicList.replaceChildren(buildPresetEmptyState('No watched folders saved yet.'));
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

function renderConversionActionBar(state, elements) {
    const actionBar = buildConversionActionBar(state);

    elements.statusText.textContent = actionBar.primary;
    elements.statusText.dataset.kind = actionBar.tone;
    elements.statusMeta.textContent = actionBar.secondary;
    elements.statusMeta.hidden = !actionBar.secondary;
    elements.statusSymbol.textContent = actionBar.symbol;
    elements.statusSymbol.dataset.kind = actionBar.tone;
    elements.statusSymbol.hidden = !actionBar.symbol;
    elements.statusSpinner.classList.toggle('is-visible', actionBar.showSpinner);

    elements.removeAllButton.hidden = !actionBar.showClear;
    elements.removeAllButton.disabled = state.isProcessing || state.isImporting;
    elements.actionShowOutputButton.hidden = !actionBar.showFinder;
    elements.actionShowOutputButton.disabled = state.isProcessing || !state.outputDirectory;
    elements.convertButton.textContent = actionBar.convertLabel;
    elements.convertButton.disabled = actionBar.convertDisabled;
}

export function buildConversionActionBar(state) {
    const imageCount = state.images.length;
    const hasImages = imageCount > 0;
    const convertLabel = state.isProcessing
        ? 'Converting...'
        : hasImages
          ? `Convert ${pluralize('Image', imageCount)}`
          : 'Convert';
    const base = {
        tone: 'info',
        primary: '',
        secondary: '',
        symbol: '',
        showSpinner: false,
        showClear: hasImages,
        showFinder: false,
        convertLabel,
        convertDisabled:
            state.isProcessing || state.isImporting || !hasImages || Boolean(state.validationMessage),
    };

    if (state.isProcessing) {
        return {
            ...base,
            primary: `Converting ${pluralize('image', imageCount)}...`,
            secondary: 'Please keep BulkPixel open.',
            showSpinner: true,
        };
    }

    if (state.isImporting) {
        return {
            ...base,
            primary: state.status.text,
            secondary: 'Please wait while BulkPixel checks the files.',
            showSpinner: true,
        };
    }

    if (state.validationMessage) {
        return {
            ...base,
            tone: 'warning',
            primary: state.validationMessage,
            symbol: '!',
            showClear: hasImages,
        };
    }

    if (state.summary && Number(state.summary.successCount ?? 0) <= 0) {
        return buildConversionResultActionBar(state, base);
    }

    if (state.status.kind === 'error') {
        return {
            ...base,
            tone: 'error',
            primary: state.status.text,
            secondary: hasImages ? 'Review the current settings or image results.' : '',
            symbol: '×',
        };
    }

    if (state.summary) {
        return buildConversionResultActionBar(state, base);
    }

    if (hasImages) {
        const totalInputSize = getTotalInputSize(state.images);
        return {
            ...base,
            primary: buildConversionJobSummary(state),
            secondary: totalInputSize === null ? '' : `${formatBytes(totalInputSize)} input`,
            showClear: true,
        };
    }

    if (state.status.kind === 'warning') {
        return {
            ...base,
            tone: 'warning',
            primary: state.status.text,
            symbol: '!',
        };
    }

    return {
        ...base,
        primary: 'No images selected',
        secondary: 'Add images to begin.',
    };
}

function buildConversionResultActionBar(state, base) {
    const summary = state.summary;
    const successCount = Number(summary.successCount ?? 0);
    const failureCount = Number(summary.failureCount ?? 0);
    const totalCount = successCount + failureCount;

    if (successCount <= 0) {
        return {
            ...base,
            tone: 'error',
            primary: 'No images were converted.',
            secondary: 'Review the errors in the image list.',
            symbol: '×',
        };
    }

    const sizeChange = buildSummaryDeltaText(
        Number(summary.totalDeltaBytes ?? 0),
        Number(summary.totalPercentChange ?? 0),
    );
    const sizeComparison = buildSizeComparison(summary);

    if (failureCount > 0) {
        return {
            ...base,
            tone: 'warning',
            primary: `${successCount} of ${totalCount} images converted · ${failureCount} failed`,
            secondary: sizeComparison ? `${sizeComparison} · ${sizeChange}` : sizeChange,
            symbol: '!',
            showFinder: true,
        };
    }

    return {
        ...base,
        tone: 'success',
        primary: `${pluralize('image', successCount)} converted · ${sizeChange}`,
        secondary: sizeComparison,
        symbol: '✓',
        showFinder: true,
    };
}

export function buildConversionJobSummary(state) {
    return [
        pluralize('image', state.images.length),
        `${buildInputFormatSummary(state.images)} → ${normalizeFormatLabel(state.format)}`,
        buildResizeSummary(state),
    ].join(' · ');
}

export function buildInputFormatSummary(images) {
    const formats = new Set(
        images
            .map(image => normalizeFormatLabel(image.fileType))
            .filter(Boolean),
    );

    if (formats.size !== 1) {
        return 'Mixed';
    }

    return formats.values().next().value;
}

export function getTotalInputSize(images) {
    if (!images.length) {
        return null;
    }

    const sizes = images.map(image => Number(image.fileSize));
    if (sizes.some(size => !Number.isFinite(size) || size < 0)) {
        return null;
    }

    return sizes.reduce((total, size) => total + size, 0);
}

export function buildImageLibraryMeta(images) {
    const count = pluralize('Image', images.length);
    const totalSize = getTotalInputSize(images);
    return totalSize === null ? count : `${count} · ${formatBytes(totalSize)} total`;
}

export function buildResizeSummary(state) {
    if (state.resizeMode === 'width') {
        return `Width ${state.width} px`;
    }

    if (state.resizeMode === 'height') {
        return `Height ${state.height} px`;
    }

    return 'Original size';
}

function normalizeFormatLabel(value) {
    const format = String(value ?? '').trim().toUpperCase();
    return format === 'JPG' ? 'JPEG' : format;
}

function buildSizeComparison(summary) {
    const originalSize = Number(summary.totalOriginalSize);
    const convertedSize = Number(summary.totalConvertedSize);
    if (!Number.isFinite(originalSize) || originalSize < 0 || !Number.isFinite(convertedSize) || convertedSize < 0) {
        return '';
    }

    return `${formatBytes(originalSize)} → ${formatBytes(convertedSize)}`;
}

function renderPreview(state, elements) {
    if (!state.images.length) {
        elements.previewList.replaceChildren();
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
    card.className = 'preset-card preset-library-card';

    const body = document.createElement('div');
    body.className = 'preset-card-body';

    const name = document.createElement('h4');
    name.textContent = preset.name;

    const summary = document.createElement('p');
    summary.className = 'preset-card-summary';
    summary.textContent = buildPresetLibrarySummary(preset);

    const filename = document.createElement('p');
    filename.className = 'preset-card-filename';
    filename.textContent = buildPresetFilenameText(preset);

    const output = document.createElement('span');
    output.className = 'preset-card-path';
    output.title = preset.outputDirectory;
    output.textContent = preset.outputDirectory;

    const details = document.createElement('div');
    details.className = 'preset-card-details';

    const separator = document.createElement('span');
    separator.className = 'preset-card-detail-separator';
    separator.setAttribute('aria-hidden', 'true');
    separator.textContent = '·';

    details.append(filename, separator, output);
    body.append(name, summary, details);

    const actions = document.createElement('div');
    actions.className = 'preset-card-actions';
    actions.append(buildPresetActionButton('apply', preset.id, 'Use', 'preset-use-button'));

    const menuWrap = document.createElement('div');
    menuWrap.className = 'preset-action-menu-wrap';

    const menuId = `preset-actions-${String(preset.id).replace(/[^a-zA-Z0-9_-]/g, '-')}`;
    const menuTrigger = document.createElement('button');
    menuTrigger.className = 'button secondary compact preset-menu-trigger';
    menuTrigger.type = 'button';
    menuTrigger.setAttribute('aria-label', `More actions for ${preset.name}`);
    menuTrigger.setAttribute('aria-haspopup', 'menu');
    menuTrigger.setAttribute('aria-expanded', 'false');
    menuTrigger.setAttribute('aria-controls', menuId);
    menuTrigger.textContent = '···';

    const menu = document.createElement('div');
    menu.id = menuId;
    menu.className = 'preset-action-menu';
    menu.setAttribute('role', 'menu');
    menu.hidden = true;
    menu.append(
        buildPresetMenuItem('edit', preset.id, 'Edit'),
        buildPresetMenuItem('duplicate', preset.id, 'Duplicate'),
        buildPresetMenuItem('delete', preset.id, 'Delete', true),
    );

    menuWrap.append(menuTrigger, menu);
    actions.append(menuWrap);

    card.append(body, actions);
    return card;
}

function buildPresetActionButton(action, presetId, label, className) {
    const button = document.createElement('button');
    button.className = `button compact preset-action-button ${className}`;
    button.type = 'button';
    button.dataset.action = action;
    button.dataset.presetId = String(presetId);
    button.textContent = label;
    return button;
}

function buildPresetMenuItem(action, presetId, label, destructive = false) {
    const button = document.createElement('button');
    button.className = `preset-action-button preset-menu-item${destructive ? ' is-destructive' : ''}`;
    button.type = 'button';
    button.dataset.action = action;
    button.dataset.presetId = String(presetId);
    button.setAttribute('role', 'menuitem');
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
    title.textContent = directory.name;
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

function buildPresetEmptyState(message) {
    const emptyState = document.createElement('div');
    emptyState.className = 'empty-state preset-empty-state';

    const title = document.createElement('p');
    title.className = 'empty-title';
    title.textContent = message;

    emptyState.append(title);
    return emptyState;
}

function buildPreviewCard(image) {
    const result = image.result;

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
    subtitle.textContent = `${image.fileType} · ${formatDimensions(image.width, image.height)} · ${formatBytes(image.fileSize)}`;

    titleContent.append(name, subtitle);
    titleRow.append(titleContent);
    body.append(titleRow);

    if (result) {
        body.append(buildPreviewResult(image, result));
    }

    card.append(thumbWrap, body);
    return card;
}

function buildPreviewResult(image, result) {
    const resultElement = document.createElement('div');
    resultElement.className = `preview-result tone-${buildResultTone(result)}`;

    const chip = document.createElement('span');
    chip.className = 'result-chip';
    chip.textContent = result.success ? buildResultChipText(image, result) : result.message;

    resultElement.append(chip);
    return resultElement;
}

export function buildResizeInputState(state) {
    return {
        widthValue: state.width || '',
        heightValue: state.height || '',
        widthReadOnly: state.resizeMode !== 'width',
        heightReadOnly: state.resizeMode !== 'height',
    };
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

export function buildPresetLibrarySummary(preset) {
    const format = preset.format.toUpperCase();
    let resolution = 'Original size';
    if (preset.resizeMode === 'width' && preset.width) {
        resolution = `Width ${preset.width} px`;
    } else if (preset.resizeMode === 'height' && preset.height) {
        resolution = `Height ${preset.height} px`;
    }

    const quality = format === 'PNG' ? 'Lossless' : `Q${preset.quality}`;
    return `${format} · ${resolution} · ${quality}`;
}

function buildPresetFilenameText(preset) {
    const component = preset.filenameComponent?.trim();
    if (!component) {
        return 'No filename component';
    }

    const label = preset.filenameMode === 'postfix' ? 'Postfix' : 'Prefix';
    return `${label} ${component}`;
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

function buildResultChipText(image, result) {
    if (!result?.success) {
        return result?.message ?? '';
    }

    const output = [
        buildDisplayFileType(image, result),
        buildDisplayDimensions(image, result),
        formatBytes(result.convertedSize),
    ].join(' · ');
    return `${output} · ${result.message}`;
}
