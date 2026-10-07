export const states: Record<string, string> = {
  queued: "Đang chờ",
  downloading: "Đang tải",
  paused: "Đã tạm dừng",
  muxing: "Đang ghép",
  verifying: "Đang kiểm tra",
  completed: "Hoàn tất",
  failed: "Cần xử lý",
  cancelled: "Đã hủy",
};
export function bytes(n: number) {
  if (n <= 0) return "0 B";
  const unit = Math.min(3, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** unit).toFixed(unit ? 1 : 0)} ${["B", "KB", "MB", "GB"][unit]}`;
}
export function duration(n: number) {
  if (!Number.isFinite(n) || n < 0) return "—";
  return [Math.floor(n / 3600), Math.floor((n % 3600) / 60), Math.floor(n % 60)]
    .map((v) => String(v).padStart(2, "0"))
    .join(":");
}
export function percentage(done: number, total: number, state: string) {
  return state === "completed"
    ? 100
    : total > 0
      ? Math.min(99, Math.round((done / total) * 100))
      : 0;
}
