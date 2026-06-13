import { render, screen, waitFor } from "@testing-library/react"
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
    last30Days: { label: "近 30 天", tokens: 4000, costUsd: 1.75 },
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
  expect(screen.getByText("820")).toBeInTheDocument()
  expect(screen.getByText("5 小时限额")).toBeInTheDocument()
  expect(screen.getByText("周限额")).toBeInTheDocument()
  expect(screen.getByText("93%")).toBeInTheDocument()
  expect(screen.getByLabelText("周限额 剩余 75%")).toBeInTheDocument()
  expect(screen.getByText("gpt-5.5")).toBeInTheDocument()
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

test("shows friendly load errors", async () => {
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "get_log_path") return Promise.resolve("Codex 用量.log")
    return Promise.reject("Codex 会话已过期，请重新运行 `codex login`。")
  })

  render(<App />)

  expect(await screen.findByRole("alert")).toHaveTextContent("Codex 会话已过期")
})
