const SIZE_UNITS = ["B", "KB", "MB", "GB"];

export function formatBytes(bytes = 0) {
  if (!Number.isFinite(bytes)) {
    return "0 B";
  }

  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < SIZE_UNITS.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  if (unitIndex === 0) {
    return `${Math.round(value)} ${SIZE_UNITS[unitIndex]}`;
  }

  return `${value.toFixed(1)} ${SIZE_UNITS[unitIndex]}`;
}

export function formatPercent(value = 0) {
  if (!Number.isFinite(value)) {
    return "0%";
  }

  return `${Math.abs(value).toFixed(1)}%`;
}

export function formatDuration(milliseconds = 0) {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;

  return minutes > 0
    ? `${minutes}min ${remainingSeconds}sec`
    : `${remainingSeconds}sec`;
}

export function formatDate(value = "") {
  const text = String(value);
  if (text.length < 10) {
    return text || "—";
  }

  return `${text.slice(8, 10)}.${text.slice(5, 7)}.${text.slice(0, 4)}`;
}

export function formatDimensions(width, height) {
  return `${width} × ${height}`;
}

export function sanitizeNumberInput(value) {
  const digits = String(value ?? "").replace(/[^\d]/g, "");
  return digits.replace(/^0+(?=\d)/, "");
}

export function pluralize(word, count) {
  return `${count} ${word}${count === 1 ? "" : "s"}`;
}

export function buildDeletePresetConfirmation(name) {
  return ['Delete preset "', name, '"?'].join("");
}

export function buildBrandTitle(magicDirectoryChangeDetected) {
  return magicDirectoryChangeDetected
    ? "BulkPixel - Magic Directory change detected"
    : "BulkPixel";
}

export function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

export function buildSummaryDeltaText(deltaBytes, percentChange) {
  if (deltaBytes >= 0) {
    return `${formatBytes(deltaBytes)} saved · ${formatPercent(percentChange)}`;
  }

  return `${formatBytes(Math.abs(deltaBytes))} larger · ${formatPercent(percentChange)}`;
}

export function buildResultTone(result) {
  if (!result?.success) {
    return "failure";
  }

  return (result.deltaBytes ?? 0) >= 0 ? "positive" : "negative";
}
