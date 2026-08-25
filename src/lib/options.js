export const DEFAULT_PROMPT_MODE = 'strict';
export const DEFAULT_RESOLUTION = 'standard';
export const DEFAULT_RATIO = '1:1';
export const DEFAULT_ORIENTATION = 'square';

export const resolutionOptions = [
  { label: '1K', value: 'standard' },
  { label: '2K', value: '2k' },
  { label: '3K', value: '3k' },
  { label: '4K', value: '4k' },
];

export const ratioOptions = ['1:1', '3:4', '4:3', '2:3', '3:2', '9:16', '16:9'].map((value) => ({
  label: value,
  value,
}));

export const qualityOptions = [
  { label: '自动', value: 'auto' },
  { label: '低', value: 'low' },
  { label: '中', value: 'medium' },
  { label: '高', value: 'high' },
];

export const ratioOrientation = {
  '1:1': 'square',
  '3:4': 'portrait',
  '4:3': 'landscape',
  '2:3': 'portrait',
  '3:2': 'landscape',
  '9:16': 'portrait',
  '16:9': 'landscape',
};

export const imageSizePresets = {
  standard: {
    '1:1': [1024, 1024],
    '3:4': [1152, 1536],
    '4:3': [1536, 1152],
    '2:3': [1024, 1536],
    '3:2': [1536, 1024],
    '9:16': [864, 1536],
    '16:9': [1536, 864],
  },
  '2k': {
    '1:1': [2048, 2048],
    '3:4': [1536, 2048],
    '4:3': [2048, 1536],
    '2:3': [1344, 2016],
    '3:2': [2016, 1344],
    '9:16': [1152, 2048],
    '16:9': [2048, 1152],
  },
  '3k': {
    '1:1': [2464, 2464],
    '3:4': [1992, 2656],
    '4:3': [2656, 1992],
    '2:3': [1840, 2760],
    '3:2': [2760, 1840],
    '9:16': [1656, 2944],
    '16:9': [2944, 1656],
  },
  '4k': {
    '1:1': [2880, 2880],
    '3:4': [2448, 3264],
    '4:3': [3264, 2448],
    '2:3': [2336, 3504],
    '3:2': [3504, 2336],
    '9:16': [2160, 3840],
    '16:9': [3840, 2160],
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
