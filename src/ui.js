import {
    buildPrivacySummary,
    buildResultTone,
    buildSummaryDeltaText,
    filterMetadataEntries,
    formatAspectRatio,
    formatBytes,
    formatDimensions,
    formatMegapixels,
    pluralize,
} from './formatters.js';
import { initDropdowns } from './vendor/oj-designsystem/index.js';

const presetDropdownCleanups = new WeakMap();
const presetListSnapshots = new WeakMap();
const presetPickerSnapshots = new WeakMap();

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
    renderImageInspector(state, elements);
}

function renderBrand(elements) {
    elements.brandTitle.textContent = 'BulkPixel';
}

function renderGlobalWatcherStatus(state, elements) {
    const watcherStatus = buildGlobalWatcherStatus(state);
    elements.globalWatcherStatus.dataset.kind = watcherStatus.kind;
    const activeTone = watcherStatus.kind === 'error'
        ? 'error'
        : watcherStatus.kind === 'watching'
          ? 'success'
          : watcherStatus.kind === 'processing'
            ? 'info'
            : '';
    for (const tone of ['success', 'error', 'info']) {
        elements.globalWatcherStatus.classList.toggle(`oj-status-${tone}`, activeTone === tone);
    }
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
    elements.dropzone.dataset.ojState = state.dragActive
        ? 'drag-active'
        : state.isImporting
          ? 'loading'
          : 'idle';
    elements.dropzone.disabled = state.isProcessing || state.isImporting;
    elements.dropzone.setAttribute('aria-disabled', String(elements.dropzone.disabled));
    elements.dropzoneTitle.textContent = state.dragActive ? 'Drop images to add them' : 'Drop images here';

    for (const option of elements.formatOptions) {
        const isActive = option.dataset.format === state.format;
        option.checked = isActive;
        option.disabled = state.isProcessing;
    }

    for (const option of elements.resizeModeOptions) {
        const isActive = option.dataset.mode === state.resizeMode;
        option.checked = isActive;
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
            button.checked = isActive;
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

    elements.magicEnabledButton.checked = form.enabled;
    elements.magicEnabledButton.disabled = isSaving;
    elements.magicOverwriteButton.checked = form.overwrite;
    elements.magicOverwriteButton.disabled = isSaving;
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
    elements.magicActivity.dataset.ojKind = state.magicActivity.kind;

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
    const isDisabled = state.isProcessing || state.presetsLoading;
    if (presetPickerSnapshots.get(elements.presetSelectMenu) !== state.presets) {
        elements.presetSelectMenu.replaceChildren(
            buildPresetPickerItem('custom', 'Custom'),
            buildPresetPickerItem('default', 'Default'),
            ...state.presets.map(preset => buildPresetPickerItem(String(preset.id), preset.name)),
        );
        presetPickerSnapshots.set(elements.presetSelectMenu, state.presets);
    }

    for (const item of elements.presetSelectMenu.children) {
        const isSelected = item.dataset.ojValue === selectedValue;
        item.setAttribute('aria-checked', String(isSelected));
        item.disabled = isDisabled;
        if (isSelected) {
            elements.presetSelectLabel.textContent = item.textContent;
        }
    }
    elements.presetSelectTrigger.disabled = isDisabled;
}

function renderPresetForm(state, elements) {
    const form = state.presetForm;
    const isSaving = state.isPresetSaving;

    elements.presetFormTitle.textContent = form.id ? 'Edit Preset' : 'Create Preset';
    elements.presetNameInput.value = form.name;
    elements.presetNameInput.disabled = isSaving;

    for (const option of elements.presetFormatOptions) {
        const isActive = option.dataset.format === form.format;
        option.checked = isActive;
        option.disabled = isSaving;
    }

    for (const button of elements.presetResizeModeOptions) {
        const isActive = button.dataset.mode === form.resizeMode;
        button.checked = isActive;
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
        button.checked = isActive;
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
    const previous = presetListSnapshots.get(elements.presetList);
    if (previous?.presets === state.presets && previous.loading === state.presetsLoading) {
        return;
    }
    presetDropdownCleanups.get(elements.presetList)?.();
    presetDropdownCleanups.delete(elements.presetList);
    presetListSnapshots.set(elements.presetList, {
        presets: state.presets,
        loading: state.presetsLoading,
    });

    if (state.presetsLoading) {
        elements.presetList.replaceChildren(buildPresetEmptyState('Loading presets...'));
        return;
    }

    if (!state.presets.length) {
        elements.presetList.replaceChildren(buildPresetEmptyState('No presets saved yet.'));
        return;
    }

    elements.presetList.replaceChildren(...state.presets.map(buildPresetCard));
    presetDropdownCleanups.set(elements.presetList, initDropdowns(elements.presetList));
}

function renderConversionActionBar(state, elements) {
    const actionBar = buildConversionActionBar(state);

    elements.statusText.textContent = actionBar.primary;
    elements.statusText.dataset.ojKind = actionBar.tone;
    elements.statusMeta.textContent = actionBar.secondary;
    elements.statusMeta.hidden = !actionBar.secondary;
    elements.statusSymbol.replaceChildren(...(actionBar.symbol ? [buildIcon(
        actionBar.tone === 'success' ? 'check' : actionBar.tone === 'error' ? 'xmark' : 'exclamation',
    )] : []));
    elements.statusSymbol.dataset.ojKind = actionBar.tone;
    elements.statusSymbol.hidden = !actionBar.symbol;
    elements.statusSpinner.hidden = !actionBar.showSpinner;

    elements.removeAllButton.hidden = !actionBar.showClear;
    elements.removeAllButton.disabled = state.isProcessing || state.isImporting;
    elements.actionShowOutputButton.hidden = !actionBar.showFinder;
    elements.actionShowOutputButton.disabled = state.isProcessing || !state.outputDirectory;
    elements.convertButton.textContent = actionBar.convertLabel;
    elements.convertButton.setAttribute('aria-busy', String(state.isProcessing));
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

    elements.previewList.replaceChildren(
        ...state.images.map(image =>
            buildPreviewCard(
                image,
                Boolean(state.inspector?.open && state.inspector.imageId === image.id),
            ),
        ),
    );
}

function buildPresetPickerItem(value, label) {
    const item = document.createElement('button');
    item.className = 'oj-menu-item';
    item.type = 'button';
    item.dataset.ojValue = value;
    item.setAttribute('role', 'menuitemradio');
    item.setAttribute('aria-checked', 'false');
    item.tabIndex = -1;
    item.textContent = label;
    return item;
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
    card.className = 'oj-panel oj-panel-compact preset-card preset-library-card';

    const body = document.createElement('div');
    body.className = 'preset-card-body';

    const name = document.createElement('h4');
    name.className = 'oj-heading-4';
    name.textContent = preset.name;

    const summary = document.createElement('p');
    summary.className = 'oj-small oj-muted preset-card-summary';
    summary.textContent = buildPresetLibrarySummary(preset);

    const filename = document.createElement('p');
    filename.className = 'oj-small oj-muted preset-card-filename';
    filename.textContent = buildPresetFilenameText(preset);

    const output = document.createElement('span');
    output.className = 'oj-path preset-card-path';
    output.title = preset.outputDirectory;
    output.textContent = preset.outputDirectory;

    const details = document.createElement('div');
    details.className = 'preset-card-details';

    details.append(filename, output);
    body.append(name, summary, details);

    const actions = document.createElement('div');
    actions.className = 'oj-inline preset-card-actions';
    actions.append(buildPresetActionButton('apply', preset.id, 'Use', 'preset-use-button'));

    const menuWrap = document.createElement('div');
    menuWrap.className = 'oj-dropdown';
    menuWrap.dataset.ojDropdown = '';

    const menuId = `preset-actions-${String(preset.id).replace(/[^a-zA-Z0-9_-]/g, '-')}`;
    const menuTrigger = document.createElement('button');
    menuTrigger.className = 'oj-icon-button preset-menu-trigger';
    menuTrigger.type = 'button';
    menuTrigger.setAttribute('aria-label', `More actions for ${preset.name}`);
    menuTrigger.setAttribute('aria-haspopup', 'menu');
    menuTrigger.setAttribute('aria-expanded', 'false');
    menuTrigger.setAttribute('aria-controls', menuId);
    menuTrigger.dataset.ojDropdownTrigger = '';
    menuTrigger.append(buildIcon('ellipsis'));

    const menu = document.createElement('div');
    menu.id = menuId;
    menu.className = 'oj-menu';
    menu.dataset.ojDropdownMenu = '';
    menu.setAttribute('role', 'menu');
    menu.setAttribute('aria-label', `Actions for ${preset.name}`);
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
    button.className = `oj-button oj-button-primary oj-button-compact preset-action-button ${className}`;
    button.type = 'button';
    button.dataset.action = action;
    button.dataset.presetId = String(presetId);
    button.textContent = label;
    return button;
}

function buildPresetMenuItem(action, presetId, label, destructive = false) {
    const button = document.createElement('button');
    button.className = `oj-menu-item${destructive ? ' oj-menu-item-danger' : ''}`;
    button.type = 'button';
    button.dataset.action = action;
    button.dataset.ojValue = action;
    button.dataset.presetId = String(presetId);
    button.setAttribute('role', 'menuitem');
    button.textContent = label;
    return button;
}

function buildMagicPresetOption(preset, selected, disabled) {
    const label = document.createElement('label');
    label.className = 'oj-check oj-panel oj-panel-compact oj-panel-interactive magic-preset-option';
    if (selected) {
        label.dataset.ojState = 'selected';
    }

    const input = document.createElement('input');
    input.className = 'oj-checkbox';
    input.type = 'checkbox';
    input.dataset.presetId = String(preset.id);
    input.checked = selected;
    input.disabled = disabled;

    const copy = document.createElement('span');
    const name = document.createElement('strong');
    name.textContent = preset.name;
    const details = document.createElement('small');
    details.className = 'oj-small oj-muted';
    details.textContent = `${preset.format.toUpperCase()} · ${buildPresetResolutionText(preset)}`;
    copy.append(name, details);
    label.append(input, copy);
    return label;
}

function buildMagicDirectoryCard(directory, presets) {
    const card = document.createElement('article');
    card.className = 'oj-panel oj-stack magic-directory-card';

    const header = document.createElement('header');
    header.className = 'oj-toolbar magic-directory-header';
    const title = document.createElement('h4');
    title.id = `magic-directory-title-${directory.id}`;
    title.className = 'oj-heading-4 magic-directory-title';
    title.textContent = directory.name;
    card.setAttribute('aria-labelledby', title.id);
    const status = document.createElement('span');
    status.className = `oj-badge magic-directory-status${directory.enabled ? ' oj-badge-success' : ''}`;
    status.textContent = directory.enabled ? 'Watching' : 'Disabled';
    header.append(title, status);

    const path = document.createElement('span');
    path.className = 'oj-path';
    path.title = directory.path;
    path.textContent = directory.path;

    const metadata = document.createElement('dl');
    metadata.className = 'oj-definition-list magic-directory-metadata';
    const formatsLabel = document.createElement('dt');
    formatsLabel.textContent = 'Formats';
    const formats = document.createElement('dd');
    formats.textContent = directory.formats.map(format => format.toUpperCase()).join(', ');

    const overwriteLabel = document.createElement('dt');
    overwriteLabel.textContent = 'Overwrite';
    const overwrite = document.createElement('dd');
    overwrite.textContent = Boolean(directory.overwrite) ? 'Enabled' : 'Disabled';

    const presetsLabel = document.createElement('dt');
    presetsLabel.textContent = 'Presets';
    const presetsValue = document.createElement('dd');
    const selectedPresetNames = directory.presetIds
        .map(id => presets.find(preset => preset.id === id)?.name)
        .filter(Boolean);
    if (selectedPresetNames.length) {
        const presetList = document.createElement('ul');
        presetList.className = 'oj-list oj-stack magic-directory-preset-list';
        for (const name of selectedPresetNames) {
            const preset = document.createElement('li');
            preset.textContent = name;
            presetList.append(preset);
        }
        presetsValue.append(presetList);
    } else {
        presetsValue.textContent = 'No presets selected';
    }
    metadata.append(formatsLabel, formats, overwriteLabel, overwrite, presetsLabel, presetsValue);

    const actions = document.createElement('footer');
    actions.className = 'oj-cluster magic-directory-actions';
    actions.append(
        buildMagicActionButton('edit', directory.id, 'Edit', directory.name),
        buildMagicActionButton('delete', directory.id, 'Delete', directory.name),
    );
    card.append(header, path, metadata, actions);
    return card;
}

function buildMagicActionButton(action, id, label, directoryName) {
    const button = document.createElement('button');
    button.className = `oj-button oj-button-${action === 'delete' ? 'danger' : 'secondary'} magic-action-button`;
    button.type = 'button';
    button.dataset.action = action;
    button.dataset.magicDirectoryId = String(id);
    button.setAttribute('aria-label', `${label} ${directoryName}`);
    button.textContent = label;
    return button;
}

function buildPresetEmptyState(message) {
    const emptyState = document.createElement('div');
    emptyState.className = 'oj-empty-state preset-empty-state';

    const title = document.createElement('p');
    title.className = 'oj-empty-state-description';
    title.textContent = message;

    emptyState.append(title);
    return emptyState;
}

function buildPreviewCard(image, selected = false) {
    const result = image.result;

    const card = document.createElement('article');
    card.className = 'oj-card oj-panel-interactive preview-card';
    card.dataset.imageId = image.id;
    if (selected) {
        card.dataset.ojState = 'selected';
        card.setAttribute('aria-current', 'true');
    }

    const thumbWrap = document.createElement('div');
    thumbWrap.className = 'thumb-wrap';

    const previewImage = document.createElement('img');
    previewImage.src = image.previewDataUrl;
    previewImage.alt = `${image.name} preview`;

    const removeButton = document.createElement('button');
    removeButton.className = 'oj-icon-button thumb-remove-button remove-image-button';
    removeButton.type = 'button';
    removeButton.dataset.imageId = image.id;
    removeButton.setAttribute('aria-label', `Remove ${image.name}`);

    removeButton.append(buildIcon('xmark'));

    thumbWrap.append(previewImage, removeButton);

    const body = document.createElement('div');
    body.className = 'oj-card-body preview-body';

    const titleRow = document.createElement('div');
    titleRow.className = 'preview-title-row';

    const titleContent = document.createElement('div');

    const name = document.createElement('h3');
    name.className = 'oj-heading-4 preview-name';
    name.title = image.name;
    name.textContent = image.name;

    const subtitle = document.createElement('p');
    subtitle.className = 'oj-caption oj-muted preview-subtitle';
    subtitle.textContent = `${image.fileType} · ${formatDimensions(image.width, image.height)} · ${formatBytes(image.fileSize)}`;

    titleContent.append(name, subtitle);

    const infoButton = document.createElement('button');
    infoButton.className = 'oj-icon-button image-info-button';
    infoButton.type = 'button';
    infoButton.dataset.imageId = image.id;
    infoButton.setAttribute('aria-label', `Show image information for ${image.name}`);
    infoButton.append(buildIcon('circle-info'));

    titleRow.append(titleContent, infoButton);
    body.append(titleRow);

    if (result) {
        body.append(buildPreviewResult(image, result));
    }

    card.append(thumbWrap, body);
    return card;
}

function renderImageInspector(state, elements) {
    const inspector = state.inspector ?? { open: false };
    const image = state.images.find(item => item.id === inspector.imageId);
    const isOpen = Boolean(inspector.open && image && state.view === 'convert');

    elements.convertView.classList.toggle('has-inspector', isOpen);
    elements.imageInspector.hidden = !isOpen;
    elements.imageInspectorScrim.hidden = !isOpen;
    elements.imageInspector.setAttribute('aria-busy', inspector.loading ? 'true' : 'false');
    elements.imageInspectorTitle.textContent = inspector.view === 'all' ? 'All Metadata' : 'Image Info';

    if (!isOpen) {
        elements.imageInspectorBody.replaceChildren();
        return;
    }

    const content = inspector.view === 'all'
        ? buildAllMetadataView(inspector)
        : buildInspectorSummary(image, inspector, state.images);
    elements.imageInspectorBody.replaceChildren(content);
}

function buildInspectorSummary(image, inspector, images) {
    const container = document.createElement('div');
    container.className = 'image-inspector-summary';

    const hero = document.createElement('div');
    hero.className = 'image-inspector-hero';
    const thumbnail = document.createElement('img');
    thumbnail.src = image.previewDataUrl;
    thumbnail.alt = `${image.name} preview`;
    const filename = document.createElement('h3');
    filename.className = 'oj-heading-4';
    filename.title = image.name;
    filename.textContent = image.name;
    const summary = document.createElement('p');
    summary.className = 'oj-caption oj-muted';
    summary.textContent = [
        formatDimensions(image.width, image.height),
        formatMegapixels(image.width, image.height),
        formatBytes(image.fileSize),
    ].join(' · ');
    hero.append(thumbnail, filename, summary);
    if (images.length > 1) {
        hero.append(buildInspectorImageNavigation(image, images));
    }
    container.append(hero);

    const metadata = inspector.data;
    const generalRows = [
        ['Format', metadata?.general?.format || image.fileType],
        metadata?.general?.mimeType && ['MIME Type', metadata.general.mimeType],
        ['Dimensions', formatDimensions(image.width, image.height)],
        ['Megapixels', formatMegapixels(image.width, image.height)],
        ['Aspect ratio', formatAspectRatio(image.width, image.height)],
        ['File size', formatBytes(image.fileSize)],
        metadata?.general?.bitDepth && ['Bit depth', `${metadata.general.bitDepth} bit`],
        metadata?.general?.channels && ['Channels', String(metadata.general.channels)],
        metadata?.general?.animated !== null
            && metadata?.general?.animated !== undefined
            && ['Animation', metadata.general.animated ? 'Yes' : 'No'],
        metadata?.general?.frameCount && ['Frames', String(metadata.general.frameCount)],
    ].filter(Boolean);
    container.append(buildInspectorSection('General', generalRows));

    if (metadata) {
        const colorRows = [
            metadata.color.colorModel && ['Color space', metadata.color.colorModel],
            [
                'ICC Profile',
                metadata.color.iccProfileChecked
                    ? metadata.color.iccProfileEmbedded
                        ? metadata.color.iccProfileName || 'Embedded profile'
                        : 'None embedded'
                    : 'Not detected',
            ],
            metadata.color.alpha !== null
                && metadata.color.alpha !== undefined
                && ['Alpha', metadata.color.alpha ? 'Yes' : 'No'],
        ].filter(Boolean);
        container.append(buildInspectorSection('Color', colorRows));
        container.append(buildMetadataSummarySection(metadata));
        container.append(buildPrivacySection(metadata.privacy));
        container.append(buildContentCredentialsSection(metadata.contentCredentials));
    }

    if (inspector.loading) {
        const loading = document.createElement('p');
        loading.className = 'oj-inline-message image-inspector-message';
        loading.textContent = 'Loading metadata...';
        container.append(loading);
    }
    if (inspector.error) {
        const error = document.createElement('p');
        error.className = 'oj-inline-message image-inspector-message';
        error.dataset.ojKind = 'error';
        error.textContent = 'Metadata could not be read.';
        error.title = inspector.error;
        container.append(error);
    }
    for (const warningText of metadata?.warnings ?? []) {
        const warning = document.createElement('p');
        warning.className = 'oj-inline-message image-inspector-message';
        warning.dataset.ojKind = 'warning';
        warning.textContent = warningText;
        container.append(warning);
    }

    if (metadata?.raw?.length) {
        const viewAllButton = document.createElement('button');
        viewAllButton.className = 'oj-button oj-button-secondary image-inspector-view-all';
        viewAllButton.type = 'button';
        viewAllButton.dataset.inspectorAction = 'view-all';
        viewAllButton.textContent = `View all metadata (${metadata.raw.length})`;
        container.append(viewAllButton);
    }

    return container;
}

function buildInspectorImageNavigation(image, images) {
    const currentIndex = images.findIndex(item => item.id === image.id);
    const navigation = document.createElement('div');
    navigation.className = 'image-inspector-navigation';
    const previous = document.createElement('button');
    previous.className = 'oj-button oj-button-ghost oj-button-compact';
    previous.type = 'button';
    previous.dataset.inspectorImageDirection = 'previous';
    previous.setAttribute('aria-label', 'Show previous image information');
    previous.append(buildIcon('arrow-left'), 'Previous');
    const position = document.createElement('span');
    position.className = 'oj-caption oj-muted';
    position.textContent = `${currentIndex + 1} of ${images.length}`;
    const next = document.createElement('button');
    next.className = 'oj-button oj-button-ghost oj-button-compact';
    next.type = 'button';
    next.dataset.inspectorImageDirection = 'next';
    next.setAttribute('aria-label', 'Show next image information');
    next.append('Next', buildIcon('arrow-right'));
    navigation.append(previous, position, next);
    return navigation;
}

function buildInspectorSection(title, rows, className = '') {
    const section = document.createElement('section');
    section.className = `image-inspector-section${className ? ` ${className}` : ''}`;
    const heading = document.createElement('h3');
    heading.className = 'oj-heading-4';
    heading.textContent = title;
    const list = document.createElement('dl');
    list.className = 'oj-key-value image-metadata-rows';
    for (const [label, value, tone] of rows) {
        list.append(buildMetadataRow(label, value, tone));
    }
    section.append(heading, list);
    return section;
}

function buildMetadataRow(label, value, tone = '') {
    const row = document.createElement('div');
    row.className = 'image-metadata-row';
    const term = document.createElement('dt');
    term.textContent = label;
    const description = document.createElement('dd');
    if (tone) {
        description.className = 'oj-inline-message';
        description.dataset.ojKind = tone;
    }
    description.textContent = String(value);
    description.title = String(value);
    row.append(term, description);
    return row;
}

function buildMetadataSummarySection(metadata) {
    const section = document.createElement('section');
    section.className = 'image-inspector-section';
    const heading = document.createElement('h3');
    heading.className = 'oj-heading-4';
    heading.textContent = 'Metadata';
    section.append(heading);

    const accordions = document.createElement('div');
    accordions.className = 'metadata-accordions';
    accordions.append(
        buildMetadataAccordion('EXIF', metadata.exif),
        buildMetadataAccordion('IPTC', metadata.iptc),
        buildMetadataAccordion('XMP', metadata.xmp),
    );
    section.append(accordions);
    return section;
}

function buildMetadataAccordion(label, group) {
    const details = document.createElement('details');
    details.className = 'oj-accordion metadata-accordion';
    const summary = document.createElement('summary');
    const name = document.createElement('span');
    name.textContent = label;
    const status = document.createElement('span');
    status.className = 'oj-caption oj-muted';
    status.textContent = metadataGroupSummary(group);
    summary.append(name, status);
    details.append(summary);

    const content = document.createElement('div');
    content.className = 'oj-accordion-body';
    if (group.error) {
        const error = document.createElement('p');
        error.className = 'oj-inline-message';
        error.dataset.ojKind = 'warning';
        error.textContent = group.error;
        content.append(error);
    } else if (group.entries.length) {
        const rows = document.createElement('dl');
        rows.className = 'oj-key-value image-metadata-rows';
        for (const entry of selectReadableEntries(group.entries)) {
            rows.append(buildMetadataRow(entry.label, entry.value));
        }
        content.append(rows);
    }
    if (content.childElementCount) {
        details.append(content);
    }
    return details;
}

function metadataGroupSummary(group) {
    if (group.status === 'unreadable') {
        return 'Unreadable';
    }
    if (group.status === 'none') {
        return 'None';
    }
    if (group.status === 'notChecked') {
        return 'Not checked';
    }
    return group.entries.length ? pluralize('field', group.entries.length) : 'Present';
}

function selectReadableEntries(entries) {
    const preferred = [
        'make', 'model', 'lens model', 'date time original', 'exposure time', 'f number',
        'photographic sensitivity', 'iso', 'focal length', 'orientation', 'creator',
        'copyright', 'headline', 'caption', 'creator tool', 'rating', 'label',
        'gps latitude', 'gps longitude',
    ];
    const selected = entries.filter(entry =>
        preferred.some(label => entry.label.toLocaleLowerCase() === label),
    );
    return (selected.length ? selected : entries).slice(0, 10);
}

function buildPrivacySection(privacy) {
    const summary = buildPrivacySummary(privacy);
    const section = document.createElement('section');
    section.className = 'image-inspector-section';
    const heading = document.createElement('h3');
    heading.className = 'oj-heading-4';
    heading.textContent = 'Privacy';
    section.append(heading);

    if (!summary.findings.length) {
        const empty = document.createElement('p');
        empty.className = 'oj-small oj-muted';
        empty.textContent = summary.emptyText;
        section.append(empty);
        return section;
    }

    const rows = document.createElement('dl');
    rows.className = 'oj-key-value image-metadata-rows';
    for (const finding of summary.findings) {
        rows.append(buildMetadataRow(finding.label, 'Present', finding.warning ? 'warning' : ''));
    }
    section.append(rows);
    return section;
}

function buildContentCredentialsSection(credentials) {
    const section = buildInspectorSection(
        'Content Credentials',
        [['C2PA', credentials.status === 'notChecked' ? 'Not checked' : credentials.status]],
    );
    const helper = document.createElement('p');
    helper.className = 'oj-helper image-inspector-section-helper';
    helper.textContent = credentials.summary;
    section.append(helper);
    return section;
}

function buildAllMetadataView(inspector) {
    const container = document.createElement('div');
    container.className = 'all-metadata-view';
    const backButton = document.createElement('button');
    backButton.className = 'oj-button oj-button-ghost oj-button-compact image-inspector-back';
    backButton.type = 'button';
    backButton.dataset.inspectorAction = 'summary';
    backButton.append(buildIcon('arrow-left'), 'Image Info');

    const search = document.createElement('input');
    search.className = 'oj-input metadata-search-input';
    search.type = 'search';
    search.placeholder = 'Search metadata...';
    search.setAttribute('aria-label', 'Search metadata');
    search.value = inspector.query;

    const modeToggle = document.createElement('div');
    modeToggle.className = 'oj-segmented metadata-view-toggle';
    modeToggle.setAttribute('role', 'radiogroup');
    modeToggle.setAttribute('aria-label', 'Metadata labels');
    modeToggle.append(
        buildMetadataModeOption('readable', 'Readable', inspector.metadataMode),
        buildMetadataModeOption('raw', 'Raw', inspector.metadataMode),
    );

    const entries = filterMetadataEntries(inspector.data?.raw ?? [], inspector.query);
    const resultCount = document.createElement('p');
    resultCount.className = 'oj-caption oj-muted metadata-result-count';
    resultCount.textContent = pluralize('result', entries.length);
    container.append(backButton, search, modeToggle, resultCount);

    if (!entries.length) {
        const empty = document.createElement('p');
        empty.className = 'oj-inline-message image-inspector-message';
        empty.textContent = inspector.query ? 'No metadata matches this search.' : 'No metadata fields found.';
        container.append(empty);
        return container;
    }

    for (const groupName of ['EXIF', 'IPTC', 'XMP']) {
        const groupEntries = entries.filter(entry => entry.group === groupName);
        if (!groupEntries.length) {
            continue;
        }
        const section = document.createElement('section');
        section.className = 'image-inspector-section all-metadata-section';
        const heading = document.createElement('h3');
        heading.className = 'oj-heading-4';
        heading.textContent = groupName;
        const rows = document.createElement('dl');
        rows.className = 'oj-key-value image-metadata-rows all-metadata-rows';
        for (const entry of groupEntries) {
            rows.append(
                buildMetadataRow(
                    inspector.metadataMode === 'raw' ? entry.key : entry.label,
                    entry.value,
                ),
            );
        }
        section.append(heading, rows);
        container.append(section);
    }
    return container;
}

function buildMetadataModeOption(mode, text, selectedMode) {
    const label = document.createElement('label');
    const input = document.createElement('input');
    input.type = 'radio';
    input.name = 'inspector-metadata-mode';
    input.dataset.inspectorMetadataMode = mode;
    input.checked = mode === selectedMode;
    const copy = document.createElement('span');
    copy.textContent = text;
    label.append(input, copy);
    return label;
}

function buildPreviewResult(image, result) {
    const resultElement = document.createElement('div');
    resultElement.className = 'preview-result';

    const chip = document.createElement('span');
    const tone = buildResultTone(result);
    chip.className = `oj-badge oj-badge-${tone === 'failure' ? 'danger' : tone === 'negative' ? 'warning' : 'success'}`;
    chip.textContent = result.success ? buildResultChipText(image, result) : result.message;

    resultElement.append(chip);
    return resultElement;
}

function buildIcon(name) {
    const icon = document.createElement('i');
    icon.className = `fa-solid fa-${name}`;
    icon.setAttribute('aria-hidden', 'true');
    return icon;
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
