import { computed, onMounted, onUnmounted, ref } from "vue";

export const GENERATION_TIMEOUT_SECONDS = 180;

const ACTIVE_STATUSES = new Set(["queued", "running", "cancelling"]);

export function useGenerationTimer(taskRef) {
  const now = ref(Date.now());
  let intervalId = 0;

  onMounted(() => {
    intervalId = window.setInterval(() => {
      now.value = Date.now();
    }, 1000);
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

  const elapsedSeconds = computed(() => {
    const startTime = Date.parse(startedAt.value);
    if (!Number.isFinite(startTime)) return 0;
    return Math.max(0, Math.floor((now.value - startTime) / 1000));
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

  const elapsedText = computed(() => formatDuration(elapsedSeconds.value));
  const timeoutText = computed(() => formatDuration(GENERATION_TIMEOUT_SECONDS));

  return {
    elapsedText,
    isTimedOut,
    isWaitingForOutput,
    label,
    timeoutText,
  };
}

function formatDuration(totalSeconds) {
  const safeSeconds = Math.max(0, totalSeconds);
  const minutes = Math.floor(safeSeconds / 60);
  const seconds = safeSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}
