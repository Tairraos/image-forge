const FILE_URI_PREFIX = "file://";

export function extractClipboardFilePaths(clipboardData) {
  if (!clipboardData) return [];
  return uniquePaths([
    ...parsePathText(readClipboardData(clipboardData, "text/uri-list")),
    ...parsePathText(readClipboardData(clipboardData, "text/plain")),
    ...Array.from(clipboardData.files || []).map((file) => file.path),
  ]);
}

export function extractDroppedFilePaths(dataTransfer) {
  if (!dataTransfer) return [];
  return uniquePaths([
    ...Array.from(dataTransfer.files || []).map((file) => file.path),
    ...parsePathText(dataTransfer.getData?.("text/uri-list")),
    ...parsePathText(dataTransfer.getData?.("text/plain")),
  ]);
}

function readClipboardData(clipboardData, type) {
  try {
    return clipboardData.getData?.(type) || "";
  } catch {
    return "";
  }
}

function parsePathText(value) {
  return String(value || "")
    .split(/\r?\n/)
    .map((line) => normalizePath(line))
    .filter(Boolean);
}

function normalizePath(value) {
  let candidate = String(value || "").trim();
  if (!candidate || candidate.startsWith("#")) return "";
  if (candidate.startsWith('"') && candidate.endsWith('"')) {
    candidate = candidate.slice(1, -1);
  }
  if (candidate.startsWith(FILE_URI_PREFIX)) {
    try {
      const url = new URL(candidate);
      if (url.protocol !== "file:") return "";
      candidate = decodeURIComponent(url.pathname);
    } catch {
      return "";
    }
  }
  if (candidate.startsWith("/") || candidate.startsWith("~/") || /^[A-Za-z]:[\\/]/.test(candidate)) {
    return candidate;
  }
  return "";
}

function uniquePaths(paths) {
  return [...new Set(paths.filter(Boolean))];
}
