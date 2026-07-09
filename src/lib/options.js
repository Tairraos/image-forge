export const DEFAULT_PROMPT_MODE = "strict";
export const DEFAULT_RESOLUTION = "standard";
export const DEFAULT_RATIO = "1:1";
export const DEFAULT_ORIENTATION = "square";

export const promptModeOptions = [
  { label: "原始模式", value: "original" },
  { label: "保真模式", value: "strict" },
  { label: "创意模式", value: "off" },
];

export const resolutionOptions = [
  { label: "1K", value: "standard" },
  { label: "2K", value: "2k" },
  { label: "4K", value: "4k" },
];

export const ratioOptions = [
  "1:1",
  "4:5",
  "5:4",
  "3:4",
  "4:3",
  "2:3",
  "3:2",
  "9:16",
  "16:9",
  "9:21",
  "21:9",
].map((value) => ({ label: value, value }));

export const qualityOptions = [
  { label: "自动", value: "auto" },
  { label: "低", value: "low" },
  { label: "中", value: "medium" },
  { label: "高", value: "high" },
];

export const ratioOrientation = {
  "1:1": "square",
  "4:5": "portrait",
  "5:4": "landscape",
  "3:4": "portrait",
  "4:3": "landscape",
  "2:3": "portrait",
  "3:2": "landscape",
  "9:16": "portrait",
  "16:9": "landscape",
  "9:21": "portrait",
  "21:9": "landscape",
};

export const imageSizePresets = {
  standard: {
    "1:1": [1024, 1024],
    "4:5": [1024, 1280],
    "5:4": [1280, 1024],
    "3:4": [1152, 1536],
    "4:3": [1536, 1152],
    "2:3": [1024, 1536],
    "3:2": [1536, 1024],
    "9:16": [864, 1536],
    "16:9": [1536, 864],
    "9:21": [672, 1568],
    "21:9": [1568, 672],
  },
  "2k": {
    "1:1": [2048, 2048],
    "4:5": [1600, 2000],
    "5:4": [2000, 1600],
    "3:4": [1536, 2048],
    "4:3": [2048, 1536],
    "2:3": [1344, 2016],
    "3:2": [2016, 1344],
    "9:16": [1152, 2048],
    "16:9": [2048, 1152],
    "9:21": [1152, 2688],
    "21:9": [2688, 1152],
  },
  "4k": {
    "1:1": [2880, 2880],
    "4:5": [2560, 3200],
    "5:4": [3200, 2560],
    "3:4": [2448, 3264],
    "4:3": [3264, 2448],
    "2:3": [2336, 3504],
    "3:2": [3504, 2336],
    "9:16": [2160, 3840],
    "16:9": [3840, 2160],
    "9:21": [1632, 3808],
    "21:9": [3808, 1632],
  },
};

export function sizeForPreset(resolution, ratio) {
  const defaultPreset = imageSizePresets[DEFAULT_RESOLUTION];
  const preset = imageSizePresets[resolution] || defaultPreset;
  const dimensions = preset[ratio] || preset[DEFAULT_RATIO] || defaultPreset[DEFAULT_RATIO];
  return `${dimensions[0]}x${dimensions[1]}`;
}

export function orientationForRatio(ratio) {
  return ratioOrientation[ratio] || DEFAULT_ORIENTATION;
}
