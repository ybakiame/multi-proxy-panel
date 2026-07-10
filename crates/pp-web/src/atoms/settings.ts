import { atom } from "jotai";
import type { Language, Theme } from "../stores/settings";

export const languageAtom = atom<Language>("zh-CN");
export const themeAtom = atom<Theme>("dark");
