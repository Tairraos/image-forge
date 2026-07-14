import { computed, onMounted, onUnmounted, ref } from "vue";

export const GENERATION_TIMEOUT_SECONDS = 300;

const ACTIVE_STATUSES = new Set(["queued", "running", "cancelling"]);

export function useGenerationTimer(taskRef) {
  const now = ref(Date.now());
  let intervalId = 0;

  onMounted(() => {
    intervalId = window.setInterval(() => {
      now.value = Date.now();
    }, 100);
  });

  onUnmounted(() => {
    window.clearInterval(intervalId);
  });

  const isWaitingForOutput = computed(() => {
    const task = taskRef.value;
    return Boolean(
      task
        && ACTIVE_STATUSES.has(task.status)
        && !(task.outputs?.length),
    );
  });

  const startedAt = computed(() => {
    const task = taskRef.value;
    if (!task) return "";
    return task.startedAt || task.createdAt || task.updatedAt || "";
  });

  const elapsedDeciseconds = computed(() => {
    const startTime = Date.parse(startedAt.value);
    if (!Number.isFinite(startTime)) return 0;
    return Math.max(0, Math.floor((now.value - startTime) / 100));
  });

  const elapsedSeconds = computed(() => {
    return Math.floor(elapsedDeciseconds.value / 10);
  });

  const isTimedOut = computed(() => {
    const task = taskRef.value;
    return Boolean(
      task
        && task.status !== "queued"
        && isWaitingForOutput.value
        && elapsedSeconds.value >= GENERATION_TIMEOUT_SECONDS,
    );
  });

  const label = computed(() => {
    const status = taskRef.value?.status;
    if (isTimedOut.value) return "超时处理中";
    if (status === "queued") return "排队中";
    if (status === "cancelling") return "取消中";
    return "生成中";
  });

  const elapsedText = computed(() => formatDuration(elapsedDeciseconds.value));

  return {
    elapsedText,
    isTimedOut,
    isWaitingForOutput,
    label,
  };
}

function formatDuration(totalDeciseconds) {
  const safeDeciseconds = Math.max(0, totalDeciseconds);
  const totalSeconds = Math.floor(safeDeciseconds / 10);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  const deciseconds = safeDeciseconds % 10;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${deciseconds}`;
}
