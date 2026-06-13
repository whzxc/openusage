import { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart"

type CodexMetric = {
  label: string
  usedPercent: number
  resetsAt?: string | null
  periodDurationMs?: number | null
}

type CodexDayUsage = {
  label: string
  tokens: number
  costUsd?: number | null
}

type CodexModelUsage = {
  name: string
  tokens: number
  percent: number
}

type CodexLocalUsageSummary = {
  today: CodexDayUsage
  yesterday: CodexDayUsage
  last30Days: CodexDayUsage
  models: CodexModelUsage[]
}

type CodexUsageSnapshot = {
  plan?: string | null
  session?: CodexMetric | null
  weekly?: CodexMetric | null
  reviews?: CodexMetric | null
  creditsRemaining?: number | null
  creditsUsd?: number | null
  resetCreditsAvailable?: number | null
  localUsage?: CodexLocalUsageSummary | null
  localUsageStatus: string
  fetchedAt: string
}

const REFRESH_INTERVAL_MS = 5 * 60 * 1000

function App() {
  const [snapshot, setSnapshot] = useState<CodexUsageSnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [autostartEnabled, setAutostartEnabled] = useState(false)
  const [autostartLoading, setAutostartLoading] = useState(false)
  const [logPath, setLogPath] = useState<string | null>(null)
  const [animationKey, setAnimationKey] = useState(0)

  const loadSnapshot = useCallback(async (manual: boolean) => {
    if (manual) {
      setRefreshing(true)
    } else {
      setLoading(true)
    }

    try {
      const command = manual ? "refresh_codex_usage" : "get_codex_usage"
      const nextSnapshot = await invoke<CodexUsageSnapshot>(command)
      setSnapshot(nextSnapshot)
      setError(null)
    } catch (rawError) {
      const message = errorMessage(rawError)
      console.error("Failed to load Codex usage:", message)
      setError(message)
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }, [])

  useEffect(() => {
    void loadSnapshot(false)
    const interval = window.setInterval(() => {
      void loadSnapshot(false)
    }, REFRESH_INTERVAL_MS)

    return () => {
      window.clearInterval(interval)
    }
  }, [loadSnapshot])

  useEffect(() => {
    void isAutostartEnabled()
      .then(setAutostartEnabled)
      .catch((rawError) => {
        const message = errorMessage(rawError)
        console.warn("Failed to read autostart state:", message)
      })

    void invoke<string>("get_log_path")
      .then(setLogPath)
      .catch((rawError) => {
        const message = errorMessage(rawError)
        console.warn("Failed to read log path:", message)
      })
  }, [])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined

    void listen("panel-will-show", () => {
      setAnimationKey((key) => key + 1)
    }).then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten()
      } else {
        unlisten = nextUnlisten
      }
    })

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  const handleRefresh = useCallback(() => {
    void loadSnapshot(true)
  }, [loadSnapshot])

  const handleAutostartChange = useCallback(async () => {
    setAutostartLoading(true)
    try {
      if (autostartEnabled) {
        await disableAutostart()
        setAutostartEnabled(false)
      } else {
        await enableAutostart()
        setAutostartEnabled(true)
      }
    } catch (rawError) {
      const message = errorMessage(rawError)
      console.error("Failed to update autostart:", message)
      setError(message)
    } finally {
      setAutostartLoading(false)
    }
  }, [autostartEnabled])

  const localUsage = snapshot?.localUsage
  const limitMetrics = useMemo(
    () => [snapshot?.session, snapshot?.weekly, snapshot?.reviews].filter(Boolean) as CodexMetric[],
    [snapshot],
  )
  const visibleModels = localUsage?.models.slice(0, 2) ?? []

  return (
    <main className="panel" data-testid="codex-panel">
      <section className="panel-card" key={animationKey}>
        <header className="panel-header">
          <div>
            <p className="eyebrow">OpenAI Codex</p>
            <h1>Codex 用量</h1>
          </div>
          <button
            className="button"
            type="button"
            onClick={handleRefresh}
            disabled={loading || refreshing}
          >
            {refreshing ? "刷新中" : "刷新"}
          </button>
        </header>

        {error ? (
          <ErrorState message={error} />
        ) : loading && !snapshot ? (
          <LoadingState />
        ) : snapshot ? (
          <>
            <section className="summary-grid" aria-label="Codex Summary">
              <SummaryTile label="计划" value={snapshot.plan ?? "未知"} />
              <SummaryTile
                label="额度"
                value={formatOptionalNumber(snapshot.creditsRemaining)}
                detail={formatCreditValue(snapshot.creditsUsd)}
              />
              <SummaryTile
                label="重置次数"
                value={formatOptionalNumber(snapshot.resetCreditsAvailable)}
              />
            </section>

            <section className="section">
              <div className="section-title">
                <h2>远程限额</h2>
                <span>{formatFetchedAt(snapshot.fetchedAt)}</span>
              </div>
              {limitMetrics.length > 0 ? (
                <div className="metric-list">
                  {limitMetrics.map((metric) => (
                    <LimitMetric key={metric.label} metric={metric} />
                  ))}
                </div>
              ) : (
                <EmptyLine label="暂无远程限额数据" />
              )}
            </section>

            <section className="section">
              <div className="section-title">
                <h2>本地 token</h2>
                <span>{localUsageStatusLabel(snapshot.localUsageStatus)}</span>
              </div>
              {localUsage ? (
                <div className="usage-grid">
                  <UsageTile usage={localUsage.today} />
                  <UsageTile usage={localUsage.yesterday} />
                  <UsageTile usage={localUsage.last30Days} />
                </div>
              ) : (
                <EmptyLine label="暂无本地 token 数据" />
              )}
            </section>

            <section className="section">
              <div className="section-title">
                <h2>模型</h2>
                <span>
                  {localUsage?.models.length
                    ? localUsage.models.length > visibleModels.length
                      ? `前 ${visibleModels.length} / ${localUsage.models.length} 个活跃`
                      : `${localUsage.models.length} 个活跃`
                    : "暂无"}
                </span>
              </div>
              {visibleModels.length ? (
                <div className="model-list">
                  {visibleModels.map((model) => (
                    <ModelRow key={model.name} model={model} />
                  ))}
                </div>
              ) : (
                <EmptyLine label="暂无模型明细" />
              )}
            </section>
          </>
        ) : null}

        <footer className="panel-footer">
          <label className="switch-row">
            <input
              type="checkbox"
              checked={autostartEnabled}
              disabled={autostartLoading}
              onChange={handleAutostartChange}
            />
            <span>开机启动</span>
          </label>
          {logPath ? <span className="log-path" title={logPath}>日志已就绪</span> : null}
        </footer>
      </section>
    </main>
  )
}

function SummaryTile({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <article className="summary-tile">
      <span>{label}</span>
      <strong>{value}</strong>
      {detail ? <small>{detail}</small> : null}
    </article>
  )
}

function LimitMetric({ metric }: { metric: CodexMetric }) {
  const remainingPercent = clampPercent(100 - metric.usedPercent)

  return (
    <article className="limit-metric">
      <div className="metric-copy">
        <span>{metric.label}</span>
        <strong>{formatPercent(remainingPercent)}</strong>
      </div>
      <div className="progress-track" aria-label={`${metric.label} 剩余 ${formatPercent(remainingPercent)}`}>
        <div className="progress-fill" style={{ width: `${remainingPercent}%` }} />
      </div>
      <small>{metric.resetsAt ? `${formatReset(metric.resetsAt)} 重置` : "重置时间未知"}</small>
    </article>
  )
}

function UsageTile({ usage }: { usage: CodexDayUsage }) {
  return (
    <article className="usage-tile">
      <span>{usage.label}</span>
      <strong>{formatTokens(usage.tokens)}</strong>
      <small>{formatCost(usage.costUsd)}</small>
    </article>
  )
}

function ModelRow({ model }: { model: CodexModelUsage }) {
  const percent = clampPercent(model.percent)

  return (
    <article className="model-row">
      <div>
        <strong>{model.name}</strong>
        <small>{formatTokens(model.tokens)}</small>
      </div>
      <span>{formatPercent(percent)}</span>
    </article>
  )
}

function LoadingState() {
  return (
    <section className="state-block" aria-label="正在读取 Codex 用量">
      <strong>正在读取 Codex 用量</strong>
      <span>正在读取 Codex 会话和本地 token。</span>
    </section>
  )
}

function ErrorState({ message }: { message: string }) {
  return (
    <section className="state-block error" role="alert">
      <strong>无法读取 Codex 用量</strong>
      <span>{message}</span>
    </section>
  )
}

function EmptyLine({ label }: { label: string }) {
  return <div className="empty-line">{label}</div>
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === "string") return error
  return "未知错误"
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(100, Math.max(0, value))
}

function formatPercent(value: number): string {
  return `${Math.round(value)}%`
}

function formatTokens(value: number): string {
  return new Intl.NumberFormat("en-US", {
    notation: value >= 1_000_000 ? "compact" : "standard",
    maximumFractionDigits: 1,
  }).format(value)
}

function formatCost(value?: number | null): string {
  if (value === null || value === undefined) return "费用未知"
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 2,
  }).format(value)
}

function formatCreditValue(value?: number | null): string | undefined {
  if (value === null || value === undefined) return undefined
  return `价值 ${formatCost(value)}`
}

function formatOptionalNumber(value?: number | null): string {
  if (value === null || value === undefined) return "未知"
  return new Intl.NumberFormat("en-US").format(value)
}

function formatFetchedAt(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return "刚刚获取"
  return `${date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })} 获取`
}

function formatReset(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return "稍后"
  return date.toLocaleString("zh-CN", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  })
}

function localUsageStatusLabel(status: string): string {
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

export { App }
