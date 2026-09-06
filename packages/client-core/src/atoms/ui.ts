import { atom } from "jotai";

/**
 * 跨页共享的「最近操作错误」（会话级 UI 状态，见 .agents/rules/client-state-management.md）。
 *
 * 由 start/stop/saveConfig/loadConfig 等操作写入：失败时记录错误信息，
 * 成功的 start/stop/saveConfig/loadConfig 清除（轮询成功不清，避免吞掉刚记录的错误）。
 * 消费方：Dashboard 的 Alert 与 `tun_auth_required` / `vpn_not_authorized` 门禁、Settings 的加载失败 Alert。
 */
export const lastActionErrorAtom = atom<string | null>(null);
