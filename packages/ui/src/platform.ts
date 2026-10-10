/**
 * 编译期平台常量（ADR-0013 D2）。
 *
 * 由消费方 vite `define` 注入 `__PP_PLATFORM__`（"desktop" | "mobile"），
 * 构建后本模块退化为字面量布尔，死分支经 minify DCE 裁剪，平台是
 * 构建期事实而非运行时判断。
 *
 * 声明内联于本文件（而非独立 d.ts）：apps/client 的 tsc 只 include 自身
 * src，跨包源码直出时独立声明文件不可见，内联声明随文件传播。
 */
declare const __PP_PLATFORM__: "desktop" | "mobile";

export const IS_MOBILE = __PP_PLATFORM__ === "mobile";
