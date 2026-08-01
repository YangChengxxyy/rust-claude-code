# Product

## Register

product

## Platform

web

> 注:实际界面是 ratatui 终端 TUI,无原生移动端规则书(HIG / Material)适用;`web` 在此仅表示"无原生规则书"的默认值。

## Users

主要用户是已经在使用 Claude Code、想要一个即插即用 Rust 替代品的开发者。他们的使用场景是终端,每天在其中工作数小时,核心任务是 agentic 编程——对话、编辑、规划,并期望配置与 CLI 参数零迁移成本(`~/.claude/settings.json` 直接复用)。成功标准:能替换掉 TypeScript 原版,成为日常主力工具,能力对齐。

## Product Purpose

用 Rust 从零重实现 Claude Code(async-first,Cargo workspace:core / api / tools / cli / tui),对齐其对话、编辑、规划能力边界。存在的意义:证明 Rust 原生二进制能在完全兼容 Claude Code 生态的前提下,提供更轻、更快的体验。成功的样子:用户把 `rust-claude` 当日常驱动器,感受不到与原版的能力差距,只感受得到性能差距。

## Positioning

「更快更轻的 agent CLI」——性能即卖点。单二进制、秒启动、低占用,恰好兼容 Claude Code 生态。每个界面决策都要强化这个主张:轻量不是配置项,是产品本身。

## Brand Personality

可靠、透明、工程感。像一个运转良好的构建系统:不喧哗、不猜谜,每一步状态都摆在明处。文案直接、具体、无营销腔;错误信息说清楚发生了什么、下一步能做什么。

## Anti-references

- 过度装饰的 TUI:启动 ASCII 大 logo、花哨边框、满屏色块——装饰掩盖信息。
- Electron 式重型终端工具:内存高、启动慢、风扇狂转——直接背叛性能定位。
- 随手拼的不一致 UI:颜色随手用、信息密度忽高忽低、状态栏时有时无——没有系统感。

## Design Principles

1. 性能是可感知的特性。启动耗时、渲染延迟、内存占用都是界面的一部分;每个视觉决策先过「这会让它变慢或变重吗」这一关。
2. 透明优先于装饰。agent 在做什么、做到哪一步、为什么停下,必须一眼可见;任何装饰不允许和信息抢注意力。
3. 一套系统,多处投影。颜色、字形符号(⏺ / • / ⎿)、间距全部来自同一套 token;dark / light / 自定义 theme.json 只是同一系统的不同投影,不允许单个界面私自造色。
4. 默认即熟悉。Claude Code 老用户零学习成本:配置、参数、交互惯性全部延续;任何新交互都必须符合旧用户已有的预期。
5. 信息永不只靠颜色传达。语义必须有颜色以外的载体(符号、字重、位置),这是终端可访问性的底线。

## Accessibility & Inclusion

做到终端平台的合理上限:正文与状态文字对比度 ≥ 4.5:1(dark / light 两套主题都满足);diff 的增删除红绿外同时用 `+` / `-` 符号区分,任何语义不单独依赖颜色;spinner 与流式渲染提供 reduced-motion 降级;自定义 `theme.json` 作为用户侧的最终自适应出口。
