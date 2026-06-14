import { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart"
import {
  calculateMetricPace,
  clampPercent,
  formatAutoRefreshCountdown,
  formatCost,
  formatFixedReset,
  formatOptionalNumber,
  formatPercent,
  formatResetCountdown,
  formatTokens,
  localUsageStatusLabel,
} from "./usageFormat"

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
  last7Days?: CodexDayUsage | null
  last30Days: CodexDayUsage
  models: CodexModelUsage[]
}

type CodexUsageSnapshot = {
  plan?: string | null
  session?: CodexMetric | null
  weekly?: CodexMetric | null
  resetCreditsAvailable?: number | null
  localUsage?: CodexLocalUsageSummary | null
  localUsageStatus: string
  fetchedAt: string
}

type ResetDisplayMode = "relative" | "fixed"

const REFRESH_INTERVAL_MS = 5 * 60 * 1000
const COUNTDOWN_INTERVAL_MS = 1000

function App() {
  const [snapshot, setSnapshot] = useState<CodexUsageSnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [autostartEnabled, setAutostartEnabled] = useState(false)
  const [autostartLoading, setAutostartLoading] = useState(false)
  const [panelOpen, setPanelOpen] = useState(() => !isTauriRuntime())
  const [nowMs, setNowMs] = useState(() => Date.now())
  const [resetDisplayMode, setResetDisplayMode] = useState<ResetDisplayMode>("relative")

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
  }, [])

  useEffect(() => {
    let disposed = false
    let animationFrame: number | undefined
    let unlistenShow: (() => void) | undefined
    let unlistenHide: (() => void) | undefined

    const cancelAnimationFrameIfNeeded = () => {
      if (animationFrame !== undefined) {
        window.cancelAnimationFrame(animationFrame)
        animationFrame = undefined
      }
    }

    void listen("panel-did-show", () => {
      cancelAnimationFrameIfNeeded()
      setPanelOpen(false)
      animationFrame = window.requestAnimationFrame(() => {
        if (!disposed) {
          setPanelOpen(true)
        }
        animationFrame = undefined
      })
    }).then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten()
      } else {
        unlistenShow = nextUnlisten
      }
    })

    void listen("panel-did-hide", () => {
      cancelAnimationFrameIfNeeded()
      setPanelOpen(false)
    }).then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten()
      } else {
        unlistenHide = nextUnlisten
      }
    })

    return () => {
      disposed = true
      cancelAnimationFrameIfNeeded()
      unlistenShow?.()
      unlistenHide?.()
    }
  }, [])

  useEffect(() => {
    const interval = window.setInterval(() => {
      setNowMs(Date.now())
    }, COUNTDOWN_INTERVAL_MS)

    return () => {
      window.clearInterval(interval)
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

  const handleQuit = useCallback(() => {
    void invoke("quit_app").catch((rawError) => {
      const message = errorMessage(rawError)
      console.error("Failed to quit app:", message)
      setError(message)
    })
  }, [])

  const localUsage = snapshot?.localUsage
  const tokenRows = useMemo(() => {
    if (!localUsage) return []
    return [localUsage.today, localUsage.last7Days, localUsage.last30Days].filter(
      Boolean,
    ) as CodexDayUsage[]
  }, [localUsage])
  const visibleModels = localUsage?.models.slice(0, 2) ?? []

  return (
    <main className={`panel ${panelOpen ? "is-open" : "is-entering"}`} data-testid="codex-panel">
      <section className="panel-card">
        <header className="panel-header">
          <div className="title-stack">
            <p className="eyebrow">OpenAI Codex</p>
            <div className="title-row">
              <h1>Codex 用量</h1>
              {snapshot ? (
                <div className="tag-row" aria-label="Codex Account Summary">
                  <span className="tag">{snapshot.plan ?? "未知计划"}</span>
                  <span className="tag">{formatResetCredits(snapshot.resetCreditsAvailable)}</span>
                </div>
              ) : null}
            </div>
          </div>
          <div className="refresh-stack">
            <button
              className="button"
              type="button"
              onClick={handleRefresh}
              disabled={loading || refreshing}
            >
              {refreshing ? "刷新中" : "刷新"}
            </button>
            {snapshot ? (
              <span className="auto-refresh" data-testid="auto-refresh-countdown">
                {formatAutoRefreshCountdown(snapshot.fetchedAt, nowMs, REFRESH_INTERVAL_MS)}
              </span>
            ) : null}
          </div>
        </header>

        {error ? (
          <ErrorState message={error} />
        ) : loading && !snapshot ? (
          <LoadingState />
        ) : snapshot ? (
          <>
            <section className="section limit-section" aria-label="Codex Limit Summary">
              <div className="section-title">
                <h2>限额</h2>
                <div className="segmented" aria-label="重置时间展示模式">
                  <button
                    type="button"
                    className={resetDisplayMode === "relative" ? "active" : ""}
                    aria-pressed={resetDisplayMode === "relative"}
                    onClick={() => setResetDisplayMode("relative")}
                  >
                    剩余时间
                  </button>
                  <button
                    type="button"
                    className={resetDisplayMode === "fixed" ? "active" : ""}
                    aria-pressed={resetDisplayMode === "fixed"}
                    onClick={() => setResetDisplayMode("fixed")}
                  >
                    固定时间
                  </button>
                </div>
              </div>
              <div className="limit-stack">
                {snapshot.weekly ? (
                  <LimitMetric
                    metric={snapshot.weekly}
                    nowMs={nowMs}
                    resetDisplayMode={resetDisplayMode}
                    variant="primary"
                    testId="weekly-limit"
                  />
                ) : (
                  <EmptyLine label="暂无周限额数据" />
                )}
                {snapshot.session ? (
                  <LimitMetric
                    metric={snapshot.session}
                    nowMs={nowMs}
                    resetDisplayMode={resetDisplayMode}
                    variant="secondary"
                    testId="session-limit"
                  />
                ) : null}
              </div>
            </section>

            <section className="section token-section">
              <div className="section-title">
                <h2>Token 消耗</h2>
                <span>{localUsageStatusLabel(snapshot.localUsageStatus)}</span>
              </div>
              {tokenRows.length ? (
                <div className="token-list">
                  {tokenRows.map((usage) => (
                    <TokenRow key={usage.label} usage={usage} />
                  ))}
                </div>
              ) : (
                <EmptyLine label="暂无本地 token 数据" />
              )}
            </section>

            {visibleModels.length ? (
              <section className="model-strip" aria-label="模型">
                <span>模型</span>
                <div>
                  {visibleModels.map((model) => (
                    <ModelChip key={model.name} model={model} />
                  ))}
                </div>
              </section>
            ) : null}
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
          <button className="footer-button" type="button" onClick={handleQuit}>
            退出
          </button>
        </footer>
      </section>
    </main>
  )
}

function LimitMetric({
  metric,
  nowMs,
  resetDisplayMode,
  variant,
  testId,
}: {
  metric: CodexMetric
  nowMs: number
  resetDisplayMode: ResetDisplayMode
  variant: "primary" | "secondary"
  testId: string
}) {
  const usedPercent = clampPercent(metric.usedPercent)
  const remainingPercent = clampPercent(100 - usedPercent)
  const pace = calculateMetricPace(metric, nowMs)
  const resetText =
    resetDisplayMode === "fixed"
      ? formatFixedReset(metric.resetsAt)
      : formatResetCountdown(metric.resetsAt, nowMs)

  return (
    <article className={`limit-metric ${variant}`} data-testid={testId}>
      <div className="metric-copy">
        <div>
          <span>{metric.label}</span>
          <small>已用 {formatPercent(usedPercent)}</small>
        </div>
        <strong>{formatPercent(remainingPercent)} 剩余</strong>
      </div>
      <div className="progress-track" aria-label={`${metric.label} 剩余 ${formatPercent(remainingPercent)}`}>
        <div className="progress-fill" style={{ width: `${remainingPercent}%` }} />
        {pace ? (
          <span
            className="progress-marker"
            aria-label={`${metric.label} 时间进度 ${formatPercent(pace.elapsedPercent)}`}
            style={{ left: `${pace.timeRemainingPercent}%` }}
          />
        ) : null}
      </div>
      <div className="metric-reset-row">
        <small>{resetText}</small>
        {pace ? <small>{pace.label}</small> : null}
      </div>
    </article>
  )
}

function TokenRow({ usage }: { usage: CodexDayUsage }) {
  return (
    <article className="token-row">
      <span>{usage.label}</span>
      <strong>{formatTokens(usage.tokens)}</strong>
      <small>{formatCost(usage.costUsd)}</small>
    </article>
  )
}

function ModelChip({ model }: { model: CodexModelUsage }) {
  return (
    <span className="model-chip">
      <span>{model.name}</span>
      <strong>{formatPercent(clampPercent(model.percent))}</strong>
    </span>
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

function formatResetCredits(value?: number | null): string {
  if (value === null || value === undefined) return "可重置 未知"
  return `可重置 ${formatOptionalNumber(value)} 次`
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === "string") return error
  return "未知错误"
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
}

export { App }
