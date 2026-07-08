export const sizeOptions = [
  { label: "1024 x 1024", value: "1024x1024" },
  { label: "1536 x 1024", value: "1536x1024" },
  { label: "1024 x 1536", value: "1024x1536" },
  { label: "Auto", value: "auto" },
];

export const qualityOptions = ["auto", "high", "medium", "low"].map((value) => ({ label: value, value }));

export const formatOptions = ["png", "jpeg", "webp"].map((value) => ({ label: value, value }));

export const backgroundOptions = [
  { label: "默认", value: "" },
  { label: "auto", value: "auto" },
  { label: "transparent", value: "transparent" },
  { label: "opaque", value: "opaque" },
];

export const fidelityOptions = [
  { label: "默认", value: "" },
  { label: "high", value: "high" },
  { label: "low", value: "low" },
];
