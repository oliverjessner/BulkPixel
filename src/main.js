import { formatBytes, formatPercent, pluralize, sanitizeNumberInput } from './formatters.js';
import { renderApp } from './ui.js';

const { invoke } = window.__TAURI__.core;
const dialogApi = window.__TAURI__.dialog;
const eventApi = window.__TAURI__.event;
const webviewApi = window.__TAURI__.webview;

const MAX_RESIZE_DIMENSION = 9999;

const state = {
    view: 'convert',
    images: [],
    results: [],
    summary: null,
    format: 'jpeg',
    width: '',
    height: '',
    resizeMode: 'none',
    resizeReference: null,
    quality: 100,
    filenameComponent: '',
    filenameMode: 'prefix',
    outputDirectory: '',
    defaultOutputDirectory: '',
    presets: [],
    presetsLoading: false,
    isPresetSaving: false,
    selectedPresetId: 'custom',
    presetForm: buildEmptyPresetForm(),
    isProcessing: false,
    isImporting: false,
    dragActive: false,
    validationMessage: '',
    status: {
        kind: 'info',
        text: 'Choose images to begin.',
    },
};

const elements = {};

window.addEventListener('DOMContentLoaded', async () => {
    cacheElements();
    bindEvents();
    render();
    await bindOpenedFiles();
    await hydrateDefaultOutputDirectory();
    resetPresetFormToCurrentSettings();
    await loadPresets();
    await bindNativeDragDrop();
});

function cacheElements() {
    elements.convertView = document.querySelector('#convert-view');
    elements.presetsView = document.querySelector('#presets-view');
    elements.convertActionBar = document.querySelector('#convert-action-bar');
    elements.appModeButtons = [...document.querySelectorAll('.app-mode-button')];
    elements.dropzone = document.querySelector('#dropzone');
    elements.dropzoneTitle = document.querySelector('#dropzone-title');
    elements.presetSelect = document.querySelector('#preset-select');
    elements.formatOptions = [...document.querySelectorAll('#format-toggle .format-option')];
    elements.widthInput = document.querySelector('#width-input');
    elements.heightInput = document.querySelector('#height-input');
    elements.resizeReference = document.querySelector('#resize-reference');
    elements.resizeHelper = document.querySelector('#resize-helper');
    elements.qualitySlider = document.querySelector('#quality-slider');
    elements.qualityValue = document.querySelector('#quality-value');
    elements.qualityHelper = document.querySelector('#quality-helper');
    elements.prefixInput = document.querySelector('#prefix-input');
    elements.filenameToggle = document.querySelector('#filename-toggle');
    elements.outputPath = document.querySelector('#output-path');
    elements.chooseFolderButton = document.querySelector('#choose-folder-button');
    elements.addImagesButton = document.querySelector('#add-images-button');
    elements.removeAllButton = document.querySelector('#remove-all-button');
    elements.previewMeta = document.querySelector('#preview-meta');
    elements.previewList = document.querySelector('#preview-list');
    elements.statusSpinner = document.querySelector('#status-spinner');
    elements.statusText = document.querySelector('#status-text');
    elements.convertButton = document.querySelector('#convert-button');
    elements.presetForm = document.querySelector('#preset-form');
    elements.presetFormTitle = document.querySelector('#preset-form-title');
    elements.presetNameInput = document.querySelector('#preset-name-input');
    elements.presetFormatOptions = [...document.querySelectorAll('#preset-format-toggle .format-option')];
    elements.presetResizeModeOptions = [...document.querySelectorAll('#preset-resize-mode-toggle .toggle-button')];
    elements.presetWidthInput = document.querySelector('#preset-width-input');
    elements.presetHeightInput = document.querySelector('#preset-height-input');
    elements.presetQualitySlider = document.querySelector('#preset-quality-slider');
    elements.presetQualityValue = document.querySelector('#preset-quality-value');
    elements.presetFilenameToggle = document.querySelector('#preset-filename-toggle');
    elements.presetFilenameModeOptions = [...document.querySelectorAll('#preset-filename-toggle .toggle-button')];
    elements.presetFilenameInput = document.querySelector('#preset-filename-input');
    elements.presetOutputPath = document.querySelector('#preset-output-path');
    elements.presetChooseFolderButton = document.querySelector('#preset-choose-folder-button');
    elements.presetResetButton = document.querySelector('#preset-reset-button');
    elements.presetSaveButton = document.querySelector('#preset-save-button');
    elements.presetCount = document.querySelector('#preset-count');
    elements.presetList = document.querySelector('#preset-list');
}

function bindEvents() {
    elements.appModeButtons.forEach(button => {
        button.addEventListener('click', () => {
            state.view = button.dataset.view;
            if (state.view === 'presets' && !state.presetForm.id && !state.presetForm.name.trim()) {
                resetPresetFormToCurrentSettings();
            }
            render();
        });
    });

    elements.dropzone.addEventListener('click', () => {
        if (!state.isProcessing) {
            void pickImages();
        }
    });

    elements.addImagesButton.addEventListener('click', () => {
        void pickImages();
    });

    elements.removeAllButton.addEventListener('click', () => {
        state.images = [];
        state.filenameComponent = '';
        syncResizeReference();
        markPresetCustom();
        clearResults();
        setStatus('info', 'All images were removed.');
        render();
    });

    elements.formatOptions.forEach(option => {
        option.addEventListener('click', () => {
            if (state.isProcessing) {
                return;
            }

            state.format = option.dataset.format;
            markPresetCustom();
            clearResults();
            validateState();
            render();
        });
    });

    elements.presetSelect.addEventListener('change', event => {
        applyPresetSelection(event.target.value);
    });

    elements.resizeReference.addEventListener('click', () => {
        if (state.isProcessing || !state.resizeReference) {
            return;
        }

        state.resizeMode = 'none';
        syncResizeReference();
        markPresetCustom();
        clearResults();
        validateState();
        render();
    });

    elements.widthInput.addEventListener('input', event => {
        const nextValue = sanitizeNumberInput(event.target.value);

        if (!nextValue) {
            state.resizeMode = 'none';
            syncResizeReference();
        } else {
            state.resizeMode = 'width';
            state.width = nextValue;
            state.height = derivePairedDimension('width', nextValue);
        }

        markPresetCustom();
        clearResults();
        validateState();
        render();
    });

    elements.heightInput.addEventListener('input', event => {
        const nextValue = sanitizeNumberInput(event.target.value);

        if (!nextValue) {
            state.resizeMode = 'none';
            syncResizeReference();
        } else {
            state.resizeMode = 'height';
            state.height = nextValue;
            state.width = derivePairedDimension('height', nextValue);
        }

        markPresetCustom();
        clearResults();
        validateState();
        render();
    });

    elements.qualitySlider.addEventListener('input', event => {
        state.quality = Number(event.target.value);
        markPresetCustom();
        clearResults();
        render();
    });

    elements.prefixInput.addEventListener('input', event => {
        state.filenameComponent = event.target.value;
        markPresetCustom();
        clearResults();
        render();
    });

    elements.filenameToggle.addEventListener('click', event => {
        const button = event.target.closest('.toggle-button');
        if (!button || state.isProcessing) {
            return;
        }

        state.filenameMode = button.dataset.mode;
        markPresetCustom();
        clearResults();
        render();
    });

    elements.chooseFolderButton.addEventListener('click', () => {
        void chooseOutputDirectory();
    });

    elements.previewList.addEventListener('click', event => {
        const button = event.target.closest('.remove-image-button');
        if (!button || state.isProcessing) {
            return;
        }

        state.images = state.images.filter(image => image.id !== button.dataset.imageId);
        syncResizeReference();
        if (!state.images.length) {
            markPresetCustom();
        }
        clearResults();
        setStatus('info', 'Image removed.');
        render();
    });

    elements.convertButton.addEventListener('click', () => {
        void handleBulkConvert();
    });

    bindPresetEvents();
}

function bindPresetEvents() {
    elements.presetForm.addEventListener('submit', event => {
        event.preventDefault();
        void savePresetForm();
    });

    elements.presetResetButton.addEventListener('click', () => {
        state.presetForm = buildEmptyPresetForm({ outputDirectory: state.outputDirectory || state.defaultOutputDirectory });
        render();
    });

    elements.presetFormatOptions.forEach(option => {
        option.addEventListener('click', () => {
            if (state.isPresetSaving) {
                return;
            }

            state.presetForm.format = option.dataset.format;
            render();
        });
    });

    elements.presetResizeModeOptions.forEach(button => {
        button.addEventListener('click', () => {
            if (state.isPresetSaving) {
                return;
            }

            state.presetForm.resizeMode = button.dataset.mode;
            if (state.presetForm.resizeMode !== 'width') {
                state.presetForm.width = '';
            }
            if (state.presetForm.resizeMode !== 'height') {
                state.presetForm.height = '';
            }
            render();
        });
    });

    elements.presetNameInput.addEventListener('input', event => {
        state.presetForm.name = event.target.value;
        render();
    });

    elements.presetWidthInput.addEventListener('input', event => {
        state.presetForm.width = sanitizeNumberInput(event.target.value);
        render();
    });

    elements.presetHeightInput.addEventListener('input', event => {
        state.presetForm.height = sanitizeNumberInput(event.target.value);
        render();
    });

    elements.presetQualitySlider.addEventListener('input', event => {
        state.presetForm.quality = Number(event.target.value);
        render();
    });

    elements.presetFilenameToggle.addEventListener('click', event => {
        const button = event.target.closest('.toggle-button');
        if (!button || state.isPresetSaving) {
            return;
        }

        state.presetForm.filenameMode = button.dataset.mode;
        render();
    });

    elements.presetFilenameInput.addEventListener('input', event => {
        state.presetForm.filenameComponent = event.target.value;
        render();
    });

    elements.presetChooseFolderButton.addEventListener('click', () => {
        void choosePresetOutputDirectory();
    });

    elements.presetList.addEventListener('click', event => {
        const button = event.target.closest('.preset-action-button');
        if (!button) {
            return;
        }

        handlePresetAction(button.dataset.action, button.dataset.presetId);
    });
}

async function hydrateDefaultOutputDirectory() {
    try {
        state.outputDirectory = await invoke('get_default_output_directory');
        state.defaultOutputDirectory = state.outputDirectory;
        render();
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to load the default output folder.'));
        render();
    }
}

async function loadPresets() {
    state.presetsLoading = true;
    render();

    try {
        state.presets = await invoke('list_presets');
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to load presets.'));
    } finally {
        state.presetsLoading = false;
        render();
    }
}

async function bindOpenedFiles() {
    if (eventApi?.listen) {
        try {
            await eventApi.listen('opened-files', event => {
                const paths = normalizePathPayload(event.payload);
                if (paths.length) {
                    void addImagePaths(paths, { source: 'Finder' });
                }
            });
        } catch (error) {
            setStatus('warning', normaliseError(error, 'Finder imports are unavailable in this environment.'));
            render();
        }
    }

    await importInitialOpenedFiles();
}

async function importInitialOpenedFiles() {
    try {
        const paths = normalizePathPayload(await invoke('get_opened_files'));
        if (paths.length) {
            await addImagePaths(paths, { source: 'Finder' });
        }
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to import images opened from Finder.'));
        render();
    }
}

async function bindNativeDragDrop() {
    if (!webviewApi?.getCurrentWebview) {
        return;
    }

    try {
        const currentWebview = webviewApi.getCurrentWebview();
        await currentWebview.onDragDropEvent(event => {
            if (event.payload.type === 'over') {
                state.dragActive = true;
                render();
                return;
            }

            if (event.payload.type === 'drop') {
                state.dragActive = false;
                render();
                void addImagePaths(event.payload.paths ?? []);
                return;
            }

            state.dragActive = false;
            render();
        });
    } catch (error) {
        setStatus('warning', normaliseError(error, 'Drag and drop is unavailable in this environment.'));
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
                    name: 'Images',
                    extensions: ['jpg', 'jpeg', 'png', 'webp', 'avif'],
                },
            ],
        });
        const paths = normalizeDialogSelection(selection);

        if (!paths.length) {
            return;
        }

        await addImagePaths(paths);
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to open the image picker.'));
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
        markPresetCustom();
        clearResults();
        setStatus('info', 'Output folder updated.');
        render();
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to choose an output folder.'));
        render();
    }
}

async function choosePresetOutputDirectory() {
    try {
        const selection = await dialogApi.open({
            directory: true,
            multiple: false,
            defaultPath: state.presetForm.outputDirectory || state.outputDirectory || undefined,
        });

        const [path] = normalizeDialogSelection(selection);
        if (!path) {
            return;
        }

        state.presetForm.outputDirectory = path;
        render();
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to choose an output folder.'));
        render();
    }
}

async function savePresetForm() {
    const validationMessage = validatePresetForm();
    if (validationMessage) {
        setStatus('error', validationMessage);
        render();
        return;
    }

    state.isPresetSaving = true;
    render();

    try {
        const savedPreset = await invoke('save_preset', { request: buildPresetRequest() });
        upsertPreset(savedPreset);
        state.presetForm = buildPresetFormFromPreset(savedPreset);
        state.selectedPresetId = String(savedPreset.id);
        applyPresetToConvert(savedPreset, { showStatus: false });
        setStatus('success', `Preset "${savedPreset.name}" saved.`);
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to save preset.'));
    } finally {
        state.isPresetSaving = false;
        render();
    }
}

async function deletePresetById(id) {
    const preset = findPresetById(id);
    if (!preset) {
        return;
    }

    const confirmed = window.confirm(`Delete preset "${preset.name}"?`);
    if (!confirmed) {
        return;
    }

    try {
        await invoke('delete_preset', { id: Number(id) });
        state.presets = state.presets.filter(item => String(item.id) !== String(id));

        if (String(state.presetForm.id) === String(id)) {
            state.presetForm = buildEmptyPresetForm({ outputDirectory: state.outputDirectory || state.defaultOutputDirectory });
        }

        if (String(state.selectedPresetId) === String(id)) {
            state.selectedPresetId = 'custom';
        }

        setStatus('success', `Preset "${preset.name}" deleted.`);
        render();
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to delete preset.'));
        render();
    }
}

async function addImagePaths(paths, options = {}) {
    const source = options.source ?? '';
    const uniquePaths = [...new Set(normalizePathPayload(paths))];
    const existingPaths = new Set(state.images.map(image => image.path));
    const freshPaths = uniquePaths.filter(path => !existingPaths.has(path));

    if (!freshPaths.length) {
        if (source) {
            return;
        }

        setStatus('info', 'Those images are already loaded.');
        render();
        return;
    }

    state.isImporting = true;
    state.status = {
        kind: 'info',
        text: source
            ? `Importing ${pluralize('image', freshPaths.length)} from ${source}...`
            : `Importing ${pluralize('image', freshPaths.length)}...`,
    };
    render();

    try {
        const response = await invoke('probe_images_command', { paths: freshPaths });
        const loadedImages = response.loaded.map(image => ({
            id: createImageId(),
            ...image,
            result: null,
        }));

        state.images.push(...loadedImages);
        syncResizeReference();
        clearResults();

        if (response.rejected.length) {
            setStatus('warning', buildImportStatus(loadedImages.length, response.rejected.length, source));
        } else {
            setStatus('success', buildImportStatus(loadedImages.length, 0, source));
        }
    } catch (error) {
        setStatus('error', normaliseError(error, 'Unable to inspect the selected images.'));
    } finally {
        state.isImporting = false;
        validateState();
        render();
    }
}

async function handleBulkConvert() {
    const validationMessage = validateState();
    if (validationMessage) {
        setStatus('error', validationMessage);
        render();
        return;
    }

    const request = {
        images: state.images.map(image => ({ path: image.path })),
        format: state.format,
        resize: {
            width: state.resizeMode === 'width' && state.width ? Number(state.width) : null,
            height: state.resizeMode === 'height' && state.height ? Number(state.height) : null,
        },
        quality: state.quality,
        filenameComponent: state.filenameComponent,
        filenameMode: state.filenameMode,
        outputDir: state.outputDirectory,
    };

    state.isProcessing = true;
    state.status = {
        kind: 'info',
        text: `Converting ${pluralize('image', state.images.length)}...`,
    };
    render();

    try {
        const response = await invoke('bulk_convert_images', { request });
        const resultsByPath = new Map(response.results.map(result => [result.inputPath, result]));

        state.results = response.results;
        state.summary = response.summary;
        state.images = state.images.map(image => ({
            ...image,
            result: resultsByPath.get(image.path) ?? null,
        }));

        if (response.summary.failureCount === 0) {
            setStatus('success', buildConversionStatusText(response.summary));
        } else if (response.summary.successCount > 0) {
            setStatus('warning', buildConversionStatusText(response.summary));
        } else {
            setStatus('error', 'No images were converted. Review the results for details.');
        }
    } catch (error) {
        setStatus('error', normaliseError(error, 'Bulk conversion failed.'));
    } finally {
        state.isProcessing = false;
        validateState();
        render();
    }
}

function applyPresetSelection(value) {
    if (value === 'custom') {
        state.selectedPresetId = 'custom';
        render();
        return;
    }

    if (value === 'default') {
        applyDefaultPresetToConvert();
        return;
    }

    const preset = findPresetById(value);
    if (!preset) {
        state.selectedPresetId = 'custom';
        render();
        return;
    }

    applyPresetToConvert(preset);
}

function applyDefaultPresetToConvert() {
    state.format = 'jpeg';
    state.quality = 100;
    state.filenameComponent = '';
    state.filenameMode = 'prefix';
    state.outputDirectory = state.defaultOutputDirectory || state.outputDirectory;
    state.resizeMode = 'none';
    syncResizeReference();
    state.selectedPresetId = 'default';
    clearResults();
    validateState();
    setStatus('info', 'Default preset applied.');
    render();
}

function applyPresetToConvert(preset, options = {}) {
    state.format = preset.format;
    state.quality = Number(preset.quality);
    state.filenameComponent = preset.filenameComponent ?? '';
    state.filenameMode = preset.filenameMode;
    state.outputDirectory = preset.outputDirectory;
    applyPresetResizeToConvert(preset);
    state.selectedPresetId = String(preset.id);
    clearResults();
    validateState();

    if (options.showStatus !== false) {
        setStatus('info', `Preset "${preset.name}" applied.`);
    }

    render();
}

function applyPresetResizeToConvert(preset) {
    state.resizeMode = normalizeResizeMode(preset.resizeMode);

    if (state.resizeMode === 'width') {
        state.width = preset.width ? String(preset.width) : '';
        state.height = state.width ? derivePairedDimension('width', state.width) : '';
        return;
    }

    if (state.resizeMode === 'height') {
        state.height = preset.height ? String(preset.height) : '';
        state.width = state.height ? derivePairedDimension('height', state.height) : '';
        return;
    }

    syncResizeReference();
}

function handlePresetAction(action, presetId) {
    const preset = findPresetById(presetId);
    if (!preset) {
        return;
    }

    if (action === 'apply') {
        applyPresetToConvert(preset);
        state.view = 'convert';
        render();
        return;
    }

    if (action === 'edit') {
        state.presetForm = buildPresetFormFromPreset(preset);
        render();
        return;
    }

    if (action === 'duplicate') {
        state.presetForm = buildDuplicatePresetForm(preset);
        setStatus('info', `Preset "${preset.name}" duplicated. Save it to keep the copy.`);
        render();
        return;
    }

    if (action === 'delete') {
        void deletePresetById(presetId);
    }
}

function resetPresetFormToCurrentSettings() {
    state.presetForm = buildEmptyPresetForm({
        format: state.format,
        resizeMode: state.resizeMode,
        width: state.resizeMode === 'width' ? state.width : '',
        height: state.resizeMode === 'height' ? state.height : '',
        quality: state.quality,
        filenameComponent: state.filenameComponent,
        filenameMode: state.filenameMode,
        outputDirectory: state.outputDirectory || state.defaultOutputDirectory,
    });
}

function buildEmptyPresetForm(overrides = {}) {
    return {
        id: null,
        name: '',
        format: 'jpeg',
        resizeMode: 'none',
        width: '',
        height: '',
        quality: 100,
        filenameComponent: '',
        filenameMode: 'prefix',
        outputDirectory: '',
        ...overrides,
    };
}

function buildPresetFormFromPreset(preset) {
    return buildEmptyPresetForm({
        id: preset.id,
        name: preset.name,
        format: preset.format,
        resizeMode: normalizeResizeMode(preset.resizeMode),
        width: preset.width ? String(preset.width) : '',
        height: preset.height ? String(preset.height) : '',
        quality: Number(preset.quality),
        filenameComponent: preset.filenameComponent ?? '',
        filenameMode: preset.filenameMode,
        outputDirectory: preset.outputDirectory,
    });
}

function buildDuplicatePresetForm(preset) {
    return {
        ...buildPresetFormFromPreset(preset),
        id: null,
        name: buildDuplicatePresetName(preset.name),
    };
}

function buildDuplicatePresetName(name) {
    const baseName = `${name} Copy`;
    const existingNames = new Set(state.presets.map(preset => preset.name.toLowerCase()));

    if (!existingNames.has(baseName.toLowerCase())) {
        return baseName;
    }

    let index = 2;
    let nextName = `${baseName} ${index}`;

    while (existingNames.has(nextName.toLowerCase())) {
        index += 1;
        nextName = `${baseName} ${index}`;
    }

    return nextName;
}

function buildPresetRequest() {
    const form = state.presetForm;
    return {
        id: form.id,
        name: form.name.trim(),
        format: form.format,
        resizeMode: form.resizeMode,
        width: form.resizeMode === 'width' && form.width ? Number(form.width) : null,
        height: form.resizeMode === 'height' && form.height ? Number(form.height) : null,
        quality: Number(form.quality),
        filenameComponent: form.filenameComponent,
        filenameMode: form.filenameMode,
        outputDirectory: form.outputDirectory,
    };
}

function validatePresetForm() {
    const form = state.presetForm;

    if (form.name.trim().length <= 3) {
        return 'Preset name must be longer than 3 characters.';
    }

    if (form.resizeMode === 'width' && !isValidPresetDimension(form.width)) {
        return `Preset width must be between 1 and ${MAX_RESIZE_DIMENSION}.`;
    }

    if (form.resizeMode === 'height' && !isValidPresetDimension(form.height)) {
        return `Preset height must be between 1 and ${MAX_RESIZE_DIMENSION}.`;
    }

    if (!form.outputDirectory.trim()) {
        return 'Choose an output folder for this preset.';
    }

    return '';
}

function isValidPresetDimension(value) {
    return isValidResizeDimension(value);
}

function upsertPreset(savedPreset) {
    const existingIndex = state.presets.findIndex(preset => preset.id === savedPreset.id);

    if (existingIndex === -1) {
        state.presets = [...state.presets, savedPreset].sort(comparePresetsByName);
        return;
    }

    state.presets = state.presets
        .map(preset => (preset.id === savedPreset.id ? savedPreset : preset))
        .sort(comparePresetsByName);
}

function comparePresetsByName(left, right) {
    return left.name.localeCompare(right.name, undefined, { sensitivity: 'base' }) || left.id - right.id;
}

function findPresetById(id) {
    return state.presets.find(preset => String(preset.id) === String(id));
}

function normalizeResizeMode(value) {
    return ['width', 'height'].includes(value) ? value : 'none';
}

function markPresetCustom() {
    state.selectedPresetId = 'custom';
}

function validateState() {
    let message = '';

    if (state.resizeMode === 'width' && !isValidResizeDimension(state.width)) {
        message = `Width must be between 1 and ${MAX_RESIZE_DIMENSION}.`;
    } else if (state.resizeMode === 'height' && !isValidResizeDimension(state.height)) {
        message = `Height must be between 1 and ${MAX_RESIZE_DIMENSION}.`;
    }

    state.validationMessage = message;
    return message;
}

function isValidResizeDimension(value) {
    const numericValue = Number(value);
    return Number.isInteger(numericValue) && numericValue >= 1 && numericValue <= MAX_RESIZE_DIMENSION;
}

function clearResults() {
    state.results = [];
    state.summary = null;
    state.images = state.images.map(image => ({ ...image, result: null }));
}

function syncResizeReference() {
    if (!state.images.length) {
        state.resizeReference = null;
        state.resizeMode = 'none';
        state.width = '';
        state.height = '';
        return;
    }

    const [firstImage] = state.images;
    const mixedSizes = state.images.some(
        image => image.width !== firstImage.width || image.height !== firstImage.height,
    );

    state.resizeReference = {
        width: firstImage.width,
        height: firstImage.height,
        mixedSizes,
    };

    if (state.resizeMode === 'none') {
        state.width = String(firstImage.width);
        state.height = String(firstImage.height);
    } else if (state.resizeMode === 'width' && state.width) {
        state.height = derivePairedDimension('width', state.width);
    } else if (state.resizeMode === 'height' && state.height) {
        state.width = derivePairedDimension('height', state.height);
    }
}

function derivePairedDimension(mode, rawValue) {
    if (!state.resizeReference) {
        return '';
    }

    const numericValue = Number(rawValue);
    if (!Number.isFinite(numericValue) || numericValue <= 0) {
        return '';
    }

    if (mode === 'width') {
        const derived = Math.round((numericValue * state.resizeReference.height) / state.resizeReference.width);
        return String(Math.max(1, derived));
    }

    const derived = Math.round((numericValue * state.resizeReference.width) / state.resizeReference.height);
    return String(Math.max(1, derived));
}

function normalizeDialogSelection(selection) {
    if (!selection) {
        return [];
    }

    return Array.isArray(selection) ? selection : [selection];
}

function normalizePathPayload(payload) {
    if (!payload) {
        return [];
    }

    const values = Array.isArray(payload) ? payload : [payload];
    return values.filter(path => typeof path === 'string' && path.trim());
}

function buildImportStatus(loadedCount, rejectedCount, source) {
    const loadedText = source
        ? `${pluralize('image', loadedCount)} imported from ${source}`
        : `${pluralize('image', loadedCount)} added`;

    if (!rejectedCount) {
        return `${loadedText}.`;
    }

    const skippedText =
        rejectedCount === 1
            ? '1 file was skipped because it is not a supported image'
            : `${rejectedCount} files were skipped because they are not supported images`;

    return loadedCount ? `${loadedText}. ${skippedText}.` : `${skippedText}.`;
}

function buildConversionStatusText(summary) {
    const parts = [`${pluralize('image', summary.successCount)} converted successfully.`];

    if (summary.failureCount > 0) {
        parts.push(`${pluralize('image', summary.failureCount)} failed.`);
    }

    parts.push(buildTotalSizeChangeText(summary));
    return parts.join(' ');
}

function buildTotalSizeChangeText(summary) {
    const deltaBytes = Number(summary.totalDeltaBytes ?? 0);
    const percentChange = Number(summary.totalPercentChange ?? 0);

    if (deltaBytes >= 0) {
        return `Total saved: ${formatBytes(deltaBytes)} (${formatPercent(percentChange)}).`;
    }

    return `Total larger: ${formatBytes(Math.abs(deltaBytes))} (${formatPercent(percentChange)}).`;
}

function createImageId() {
    if (window.crypto?.randomUUID) {
        return window.crypto.randomUUID();
    }

    return `image-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function normaliseError(error, fallbackMessage) {
    if (typeof error === 'string' && error.trim()) {
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
