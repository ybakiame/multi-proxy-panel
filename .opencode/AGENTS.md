# Project Agents Guide

## 当前分层策略
- **orchestrator**（Kimi K3/DeepSeek V4 Pro）：规划 + 裁判 + 人机确认
- **executor**（DeepSeek V4 Flash）：纯执行
- **reviewer**（Kimi K3/DeepSeek V4 Pro）：执行后审查（可选）

## 使用约定
1. 默认与 orchestrator 对话。
2. 复杂任务先让它出计划，确认后再执行。
3. 执行中出问题优先让 orchestrator 判断。
4. 只有最终决策或高风险点才直接找我。

## 项目技术栈与规范
（在这里写你的真实技术栈、目录约定、命名规范、测试要求等）