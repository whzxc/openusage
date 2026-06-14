export type MetricLike = {
  usedPercent: number
  resetsAt?: string | null
  periodDurationMs?: number | null
}

export function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(100, Math.max(0, value))
}

export function formatPercent(value: number): string {
  return `${Math.round(value)}%`
}

export function formatTokens(value: number): string {
  return new Intl.NumberFormat("en-US", {
    notation: value >= 1_000 ? "compact" : "standard",
    maximumFractionDigits: 1,
  }).format(value)
}

export function formatCost(value?: number | null): string {
  if (value === null || value === undefined) return "费用未知"
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 2,
  }).format(value)
}

export function formatOptionalNumber(value?: number | null): string {
  if (value === null || value === undefined) return "未知"
  return new Intl.NumberFormat("en-US").format(value)
}

export function formatFixedReset(value?: string | null): string {
  if (!value) return "重置时间未知"
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return "重置时间未知"
  return `${date.toLocaleString("zh-CN", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  })} 重置`
}

export function formatResetCountdown(value?: string | null, nowMs = Date.now()): string {
  if (!value) return "重置时间未知"
  const resetsAtMs = Date.parse(value)
  if (!Number.isFinite(resetsAtMs) || !Number.isFinite(nowMs)) return "重置时间未知"
  const remainingMs = resetsAtMs - nowMs
  if (remainingMs <= 0) return "已重置"
  return `还剩 ${formatDuration(remainingMs)}`
}

export function formatAutoRefreshCountdown(
  fetchedAt?: string | null,
  nowMs = Date.now(),
  intervalMs = 5 * 60 * 1000,
): string {
  const fetchedAtMs = fetchedAt ? Date.parse(fetchedAt) : Number.NaN
  if (!Number.isFinite(fetchedAtMs) || !Number.isFinite(nowMs)) return "自动刷新 --:--"
  const remainingMs = Math.max(0, fetchedAtMs + intervalMs - nowMs)
  const totalSeconds = Math.ceil(remainingMs / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `自动刷新 ${minutes.toString().padStart(2, "0")}:${seconds
    .toString()
    .padStart(2, "0")}`
}

export function calculateMetricPace(metric: MetricLike, nowMs: number) {
  if (!metric.resetsAt || !metric.periodDurationMs || metric.periodDurationMs <= 0) return null
  const resetsAtMs = Date.parse(metric.resetsAt)
  if (!Number.isFinite(resetsAtMs) || !Number.isFinite(nowMs)) return null

  const elapsedMs = nowMs - (resetsAtMs - metric.periodDurationMs)
  if (elapsedMs <= 0 || nowMs >= resetsAtMs) return null

  const elapsedPercent = clampPercent((elapsedMs / metric.periodDurationMs) * 100)
  const usedPercent = clampPercent(metric.usedPercent)
  const deltaPercent = Math.round(usedPercent - elapsedPercent)

  if (deltaPercent === 0) {
    return {
      elapsedPercent,
      timeRemainingPercent: clampPercent(100 - elapsedPercent),
      label: "消耗与时间进度持平",
    }
  }

  return {
    elapsedPercent,
    timeRemainingPercent: clampPercent(100 - elapsedPercent),
    label: `消耗${deltaPercent > 0 ? "快于" : "慢于"}时间进度 ${Math.abs(deltaPercent)}%`,
  }
}

export function localUsageStatusLabel(status: string): string {
  switch (status) {
    case "ok":
      return "ccusage 就绪"
    case "no_runner":
      return "runner 缺失"
    case "runner_failed":
      return "ccusage 失败"
    default:
      return "未检查"
  }
}

function formatDuration(valueMs: number): string {
  const totalMinutes = Math.max(1, Math.ceil(valueMs / 60_000))
  const days = Math.floor(totalMinutes / 1_440)
  const hours = Math.floor((totalMinutes % 1_440) / 60)
  const minutes = totalMinutes % 60

  if (days > 0) {
    return hours > 0 ? `${days} 天 ${hours} 小时` : `${days} 天`
  }
  if (hours > 0) {
    return minutes > 0 ? `${hours} 小时 ${minutes} 分` : `${hours} 小时`
  }
  return `${minutes} 分`
}
