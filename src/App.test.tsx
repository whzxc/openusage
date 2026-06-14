import { act, render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, expect, test, vi } from "vitest"
import { invoke } from "@tauri-apps/api/core"
import { isEnabled } from "@tauri-apps/plugin-autostart"
import { App } from "./App"

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  enable: vi.fn(),
  disable: vi.fn(),
  isEnabled: vi.fn(),
  listen: vi.fn(),
}))

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
}))

vi.mock("@tauri-apps/api/event", () => ({
  listen: mocks.listen,
}))

vi.mock("@tauri-apps/plugin-autostart", () => ({
  enable: mocks.enable,
  disable: mocks.disable,
  isEnabled: mocks.isEnabled,
}))

const snapshot = {
  plan: "Pro 5x",
  session: {
    label: "5 小时限额",
    usedPercent: 7,
    resetsAt: "2026-06-13T18:00:00Z",
    periodDurationMs: 18_000_000,
  },
  weekly: {
    label: "周限额",
    usedPercent: 25,
    resetsAt: "2026-06-15T18:00:00Z",
    periodDurationMs: 604_800_000,
  },
  reviews: null,
  creditsRemaining: 820,
  creditsUsd: 32.8,
  resetCreditsAvailable: 1,
  localUsage: {
    today: { label: "今日", tokens: 3000, costUsd: 1.25 },
    yesterday: { label: "昨日", tokens: 1000, costUsd: 0.5 },
    last7Days: { label: "近 7 天", tokens: 18400, costUsd: 7.6 },
    last30Days: { label: "近 30 天", tokens: 72100, costUsd: 29.4 },
    models: [{ name: "gpt-5.5", tokens: 3000, percent: 75 }],
  },
  localUsageStatus: "ok",
  fetchedAt: "2026-06-13T12:00:00Z",
}

beforeEach(() => {
  vi.useRealTimers()
  vi.mocked(invoke).mockReset()
  vi.mocked(isEnabled).mockReset()
  mocks.enable.mockReset()
  mocks.disable.mockReset()
  mocks.listen.mockReset()
  mocks.listen.mockResolvedValue(vi.fn())
  vi.mocked(isEnabled).mockResolvedValue(false)
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "get_log_path") return Promise.resolve("Codex 用量.log")
    return Promise.resolve(snapshot)
  })
})

test("renders codex usage snapshot", async () => {
  render(<App />)

  expect(await screen.findByRole("heading", { name: "Codex 用量" })).toBeInTheDocument()
  expect(screen.getByText("Pro 5x")).toBeInTheDocument()
  expect(screen.queryByText("820")).not.toBeInTheDocument()
  expect(screen.getByText("可重置 1 次")).toBeInTheDocument()
  expect(screen.getByText("5 小时限额")).toBeInTheDocument()
  expect(screen.getByText("周限额")).toBeInTheDocument()
  expect(screen.getByText("75% 剩余")).toBeInTheDocument()
  expect(screen.getByText("93% 剩余")).toBeInTheDocument()
  expect(screen.getByLabelText("周限额 剩余 75%")).toBeInTheDocument()
  expect(screen.getByText("gpt-5.5")).toBeInTheDocument()
})

test("prioritizes weekly quota and compact token spend", async () => {
  vi.useFakeTimers({ toFake: ["Date"] })
  vi.setSystemTime(new Date("2026-06-13T17:00:00Z"))
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "get_log_path") return Promise.resolve("Codex 用量.log")
    return Promise.resolve({ ...snapshot, fetchedAt: "2026-06-13T17:00:00Z" })
  })
  render(<App />)

  expect(await screen.findByTestId("weekly-limit")).toHaveTextContent("周限额")
  expect(screen.getByTestId("weekly-limit")).toHaveTextContent("75% 剩余")
  expect(screen.getByTestId("session-limit")).toHaveTextContent("5 小时限额")
  expect(screen.getByTestId("auto-refresh-countdown")).toHaveTextContent("自动刷新 05:00")
  expect(screen.getByText("今日")).toBeInTheDocument()
  expect(screen.getByText("近 7 天")).toBeInTheDocument()
  expect(screen.getByText("近 30 天")).toBeInTheDocument()
  expect(screen.getByText("$7.60")).toBeInTheDocument()
  expect(screen.getByText("$29.40")).toBeInTheDocument()
  expect(screen.queryByText("代码评审")).not.toBeInTheDocument()
  expect(screen.queryByText("额度")).not.toBeInTheDocument()
})

test("toggles quota reset display between remaining and fixed time", async () => {
  const user = userEvent.setup()
  vi.useFakeTimers({ toFake: ["Date"] })
  vi.setSystemTime(new Date("2026-06-13T17:00:00Z"))
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "get_log_path") return Promise.resolve("Codex 用量.log")
    return Promise.resolve({ ...snapshot, fetchedAt: "2026-06-13T17:00:00Z" })
  })
  render(<App />)

  expect(await screen.findByText("还剩 2 天 1 小时")).toBeInTheDocument()

  await user.click(screen.getByRole("button", { name: "固定时间" }))

  expect(screen.getByText("6月16日 02:00 重置")).toBeInTheDocument()
  expect(screen.queryByText("还剩 2 天 1 小时")).not.toBeInTheDocument()
})

test("shows reset countdown and usage pace against elapsed time", async () => {
  vi.useFakeTimers({ toFake: ["Date"] })
  vi.setSystemTime(new Date("2026-06-13T17:00:00Z"))
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "get_log_path") return Promise.resolve("Codex 用量.log")
    return Promise.resolve({
      ...snapshot,
      session: {
        label: "5 小时限额",
        usedPercent: 60,
        resetsAt: "2026-06-13T18:30:00Z",
        periodDurationMs: 18_000_000,
      },
      reviews: {
        label: "代码评审",
        usedPercent: 80,
        resetsAt: "2026-06-13T18:30:00Z",
        periodDurationMs: 18_000_000,
      },
      weekly: null,
    })
  })

  render(<App />)

  expect(await screen.findByText("还剩 1 小时 30 分")).toBeInTheDocument()
  expect(screen.getByText("消耗慢于时间进度 10%")).toBeInTheDocument()
  expect(screen.queryByText("代码评审")).not.toBeInTheDocument()
  expect(screen.getByLabelText("5 小时限额 时间进度 70%")).toHaveStyle({ left: "30%" })
})

test("refreshes codex usage on demand", async () => {
  const user = userEvent.setup()
  render(<App />)

  await screen.findByText("Pro 5x")
  await user.click(screen.getByRole("button", { name: "刷新" }))

  await waitFor(() => {
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("refresh_codex_usage")
  })
})

test("shows a footer exit button instead of log readiness", async () => {
  const user = userEvent.setup()
  render(<App />)

  await screen.findByText("Pro 5x")

  expect(screen.queryByText("日志已就绪")).not.toBeInTheDocument()
  await user.click(screen.getByRole("button", { name: "退出" }))

  expect(vi.mocked(invoke)).toHaveBeenCalledWith("quit_app")
  expect(vi.mocked(invoke)).not.toHaveBeenCalledWith("get_log_path")
})

test("shows friendly load errors", async () => {
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "get_log_path") return Promise.resolve("Codex 用量.log")
    return Promise.reject("Codex 会话已过期，请重新运行 `codex login`。")
  })

  render(<App />)

  expect(await screen.findByRole("alert")).toHaveTextContent("Codex 会话已过期")
})

test("animates the whole panel on tray show events", async () => {
  const eventHandlers = new Map<string, () => void>()
  mocks.listen.mockImplementation((event, handler) => {
    eventHandlers.set(String(event), handler as () => void)
    return Promise.resolve(vi.fn())
  })

  render(<App />)

  await screen.findByText("Pro 5x")
  await waitFor(() => {
    expect(eventHandlers.has("panel-did-show")).toBe(true)
    expect(eventHandlers.has("panel-did-hide")).toBe(true)
  })

  const panel = screen.getByTestId("codex-panel")
  expect(panel).toHaveClass("is-open")

  act(() => {
    eventHandlers.get("panel-did-hide")?.()
  })
  expect(panel).toHaveClass("is-entering")

  act(() => {
    eventHandlers.get("panel-did-show")?.()
  })
  await waitFor(() => {
    expect(panel).toHaveClass("is-open")
  })
})
