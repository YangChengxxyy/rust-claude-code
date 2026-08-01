---
name: rust-claude-code
description: 更快更轻的 agent CLI — Claude Code 的 Rust 重实现
colors:
  claude-terracotta: "#D77757"
  bash-magenta: "#FD5DB1"
  suggestion-periwinkle: "#B1B9F9"
  plan-teal: "#48968C"
  ink: "#FFFFFF"
  inactive: "#999999"
  subtle: "#505050"
  prompt-border: "#888888"
  user-msg-bg: "#373737"
  bash-msg-bg: "#413C41"
  success: "#4EBA65"
  error: "#FF6B80"
  warning: "#FFC107"
  diff-added-bg: "#225C2B"
  diff-removed-bg: "#7A2936"
  diff-added-word: "#38A660"
  diff-removed-word: "#B3596B"
typography:
  display:
    fontFamily: "monospace (terminal-configured)"
    fontSize: "1 cell"
    fontWeight: 700
    lineHeight: 1
  body:
    fontFamily: "monospace (terminal-configured)"
    fontSize: "1 cell"
    fontWeight: 400
    lineHeight: 1
  label:
    fontFamily: "monospace (terminal-configured)"
    fontSize: "1 cell"
    fontWeight: 400
    lineHeight: 1
rounded:
  md: "1-cell rounded (border::ROUNDED ╭╮╰╯)"
spacing:
  gutter: "2 cells (消息统一缩进)"
  message-gap: "1 line (消息块之间空行)"
  status-bar: "1 line"
  side-panel: "30 cols"
components:
  status-bar:
    textColor: "{colors.inactive}"
    height: "1 line"
  input-box:
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    height: "3-8 lines (随内容伸缩)"
  dialog-permission:
    rounded: "{rounded.md}"
    width: "50 cols"
    height: "12 lines"
---

# Design System: rust-claude-code

## 1. Overview

**Creative North Star: "The Machinist's Bench 精测工位"**

这是一张精密技工的工作台:每件工具各就其位,伸手可及,台面上没有一件装饰品。界面是终端,终端即约束即美学——一格一字、没有阴影、没有渐变,所有信息密度、层级、状态都靠一套严格的 token 系统投影出来。用户是每天在其中工作数小时的开发者;界面的职责是让他们瞟一眼就知道 agent 在干什么、干到哪一步、为什么停下,然后立刻回到自己的工作。

系统明确拒绝三样东西(与 PRODUCT.md 反参照同名):过度装饰的 TUI(启动 ASCII 大 logo、花哨边框、满屏色块);Electron 式重型工具的臃肿感;随手拼的不一致 UI。任何新界面元素入场前必须回答:它属于哪一套既有 token?它和信息抢注意力了吗?

**Key Characteristics:**

- 一套 17 色 token 调色板,dark / light / 自定义 theme.json 三种投影,渲染层不允许私自造色
- 字形符号语言:`⏺`(进行中)、`•`(助手)、`⎿`(工具/结果)、`>`(用户/选中)、`⎇`(git 分支)
- 层级三件套:终端没有字号,只有 **字重 + 颜色 + 字形符号** 可用
- 扁平为底,模态为层:深度只靠 `Clear` 浮层与底色块表达
- 语义双通道:任何语义都有颜色以外的载体(符号、字重、位置)

## 2. Colors

调色板的性格:一个克制的品牌声部,加上各司其职的功能色,铺在终端自身的黑/白底上。frontmatter 为 dark 主题(默认);light 主题见文末投影表。

### Primary

- **陶土橙 Claude Terracotta** (#D77757):品牌声部。模型名、git 分支、spinner 文案(Thinking…/Streaming…)、选中态、一级标题、建议分组标题。它是界面里唯一"说话"的颜色。

### Secondary

- **品红 Bash Magenta** (#FD5DB1):命令/外壳语义专用。Bash 工具行的 `!` 前缀、代码块与 JSON 构造框的 `┌─ │ └─` 框线。它意味着"这里有一段可执行的代码结构"。

### Tertiary

- **长春花蓝 Suggestion Periwinkle** (#B1B9F9):交互提示语义专用。用户消息 `>` 前缀、斜杠建议浮层边框、列表项 marker、二级标题。它意味着"这里可以操作/输入"。
- **计划青 Plan Teal** (#48968C):plan 模式专用。plan 相关边框与标识,出现时即代表"当前处于只读规划态"。

### Neutral

- **墨色 Ink** (#FFFFFF,dark):一切正文。纯终端黑底上的最高对比。
- **灰 Inactive** (#999999):次要但需阅读的文字——工具结果、thinking 摘要、状态栏右段。黑底上约 8.6:1,达标。
- **深灰 Subtle** (#505050):占位与禁用态专用(空状态提示、锁定输入框)。**对比度不足 4.5:1,禁止用于需要阅读的文字。**
- **边框灰 Prompt Border** (#888888):输入框默认边框。
- **炭灰 User Msg BG** (#373737):用户回显消息底色。
- **Bash 底色 Bash Msg BG** (#413C41):预留 token,当前渲染层未使用。

### Semantic

- **信号绿 Success** (#4EBA65) / **信号红 Error** (#FF6B80) / **信号黄 Warning** (#FFC107):状态语义三色。Warning 兼任系统消息文字色与权限对话框边框色。
- **Diff 四色**:整行底色 diff-added-bg (#225C2B) / diff-removed-bg (#7A2936),永远与 `+` / `-` gutter 符号成对出现。词级高亮 diff-added-word (#38A660) / diff-removed-word (#B3596B) 为预留 token,当前渲染层未接入词级高亮。

### Named Rules

**The One Voice Rule 一把声音规则。** 陶土橙是唯一的品牌声部,任一屏幕占比 ≤10%。品红、长春花蓝、计划青是功能色,只允许出现在各自的语义位上——拿它们当装饰用,等于把工具放错了工位。

**The Two-Channel Rule 双通道规则。** 任何语义不得只靠颜色传达:diff 有 `+`/`-`,选中有 `>`,错误有独立前缀,锁定态改写标题文案。色盲用户关掉颜色也必须能完整使用。

### Light 主题投影(参考值)

| Token | dark | light |
| --- | --- | --- |
| claude-terracotta | #D77757 | #B45A37 |
| bash-magenta | #FD5DB1 | #B4468C |
| suggestion-periwinkle | #B1B9F9 | #556EDC |
| plan-teal | #48968C | #3C8278 |
| ink | #FFFFFF | #141414 |
| inactive | #999999 | #6E6E6E |
| subtle | #505050 | #B4B4B4 |
| prompt-border | #888888 | #969696 |
| user-msg-bg | #373737 | #EBEBEB |
| bash-msg-bg | #413C41 | #F0EBF0 |
| success | #4EBA65 | #228B22 |
| error | #FF6B80 | #C43030 |
| warning | #FFC107 | #B47800 |
| diff-added-bg / removed-bg | #225C2B / #7A2936 | #D2F5D2 / #FADCE1 |
| diff-added-word / removed-word | #38A660 / #B3596B | #3C963C / #C85A5A |

## 3. Typography

**Display / Body / Label Font:** 全部由终端模拟器的等宽字体决定(monospace,终端配置)。系统不选择字体,只选择字重、颜色与符号。

**Character:** 沉默的工程图纸。没有字号阶梯,层级全部来自粗体与颜色的克制 pairing;斜体只出现在 streaming thinking 的实时投影里,表示"这是未完成的中间态"。

### Hierarchy

- **Display**(bold,陶土橙):一级标题、模型名。每屏至多一处。
- **Headline**(bold,长春花蓝):二级标题、建议浮层分组标题(Commands / Skills)。
- **Title**(bold,墨色):三级及以下标题、工具名(如 `Bash`、`FileEdit`)。
- **Body**(regular,墨色):正文与命令文本。所有消息统一 2 格 gutter 缩进,续行对齐。
- **Label**(regular,灰 Inactive):状态栏、工具结果、摘要、计数(`… (3 more lines)`)。斜体变体仅用于 streaming thinking。
- **Glyph**(inline):`⏺` 进行中(spinner 色)、`•` 助手消息首行、`⎿` 工具/结果行、`>` 用户与选中行、`⎇` git 分支、`┌─ │ └─` 代码框线。字形即组件,不是装饰。

### Named Rules

**The Weight-Is-Hierarchy Rule 字重即层级规则。** 终端里不存在"更大的字"。需要层级时,先加粗,再换色,最后才考虑新符号——三者不得同时堆叠超过两层。

## 4. Elevation

系统完全扁平:终端没有阴影。深度只有两种合法表达——其一,模态浮层用 `Clear` 部件把下层内容整块盖住(权限、信任、提问、会话选择四类对话框,以及斜杠建议浮层);其二,语义底色块(用户消息炭灰底、diff 整行底色)。不存在第三种"层级感"手法,禁止用颜色深浅假装阴影。

### Named Rules

**The Flat-By-Default Rule 默认扁平规则。** 静止的界面没有深度。深度只在两种状态出现:模态打断(Clear 浮层)与语义归属(底色块)。如果你在想"加个层次",答案永远是用间距和分组,不是用假阴影。

## 5. Components

### 消息行(Message Rows)

- **用户消息**:`>` 前缀(长春花蓝,bold)+ 正文(墨色),炭灰底(#373737);消息块之间空 1 行。
- **助手消息**:首行 `•` 项目符,markdown 渲染;标题/列表/代码块按层级三件套着色。
- **Thinking 块**:可折叠行,`Thinking — 摘要 [Tab to expand]`;标签 bold 陶土橙,摘要灰。流式中间态为 `⏺ [Thinking]`,灰 + 斜体。
- **空状态**:仅一行 `Type a message below to get started.`(深灰 Subtle)。

### 工具调用与结果行(Tool Lines)

- **调用行**:`⎿ ToolName (摘要)`,工具名 bold;Bash 额外获得品红 `!` 前缀;命令超 160 字符截断加 `…`。
- **构造中流式投影**:`⏺ ToolName constructing...`,部分 JSON 收入 `┌─ input │` 框(品红)。
- **结果行**:` ⎿ ` 前缀 + 灰色文本,至多 6 行,超出折叠为 `… (N more lines)`;错误改信号红。结果永远比正文暗一档,不得与助手文本抢视觉。

### 代码块框(Code Frame)

- `┌─ lang` 起、`│` 续、`└─` 止,框线用品红;块内 syntect 语法高亮(scope 色为内置取值,dark / light 两套;当前不随自定义 theme.json 变化)。

### Diff 视图

- gutter 为 `+`/`-`/空格 + 新旧行号(`+  12  34 |`),整行铺底色(绿底 #225C2B / 红底 #7A2936)。超过 20 行折叠为前 10 + 计数行 + 后 5。

### 输入框(Input)

- 圆角边框(1-cell rounded,边框灰),标题 ` Input `;高度 3–8 行随内容伸缩。流式期间锁定:边框与文字降深灰,标题改写为 ` Input (locked) `——状态变更永远改文案,不只改色。

### 建议浮层(Slash Suggestions)

- 圆角边框(长春花蓝),标题 ` Suggestions `,从输入框上方向上浮出(`Clear` 盖层),宽 ≤90 列。分组标题(Commands / Skills)陶土橙 bold;选中行 `>` + 陶土橙 bold,未选中墨色。

### 状态栏(Status Bar)

- 单行,左:模型名 + `⎇ 分支`(陶土橙 bold);右:`tokens: N↑ N↓ cache:% | mode | theme:dark`(灰)。左右之间空白填充,不画分隔线。

### 模态对话框(Dialogs)

- 居中,圆角边框,`Clear` 盖层。权限对话框 50×12,信号黄边框,标题居中 ` Permission Required `;选项列表选中行用 `>` 前缀。所有对话框共用同一套几何与边框语言。

### Todo 侧栏(Side Panel)

- 右侧固定 30 列,主区保留 Min(40);边框计划青。出现时主区让位,不遮挡。

## 6. Do's and Don'ts

### Do

- **Do** 一切颜色取自 `theme.rs` 的 Palette token;dark / light / theme.json 三种投影自动生效。
- **Do** 用语义双通道:diff 永远 `+`/`-` 与底色同行,锁定态永远改写标题文案,选中永远有 `>`。
- **Do** 长输出折叠并给计数(`… (N more lines)` / 前 10 + 后 5),保持信息密度稳定。
- **Do** 结果与次要信息用灰(Inactive,#999999),让它比正文暗一档。
- **Do** 新对话框沿用同一几何:居中、圆角、`Clear` 盖层、居中标题。

### Don't

- **Don't** 出现过度装饰的 TUI——启动 ASCII 大 logo、花哨边框、满屏色块,一律禁止(PRODUCT.md 反参照,原文引用)。
- **Don't** 做出 Electron 式重型工具的感觉:任何渲染路径引入可感知的卡顿、闪烁、重绘浪费,等同于视觉事故(PRODUCT.md 反参照,原文引用)。
- **Don't** 随手拼无系统感的 UI:渲染层私自造色(`Color::Rgb(...)` 字面量)、一次性符号、忽高忽低的密度(PRODUCT.md 反参照,原文引用)。
- **Don't** 把深灰 Subtle(#505050)用于需要阅读的文字——它只属于占位与禁用态;可读次要信息用灰 Inactive。
- **Don't** 用品红 / 长春花蓝 / 计划青做装饰:它们是功能色,离开语义位即违规(The One Voice Rule)。
- **Don't** 用颜色深浅假装阴影或层级:终端只有扁平、浮层、底色块三种深度(The Flat-By-Default Rule)。
- **Don't** 堆叠超过两层层级手段(bold + 换色 + 新符号同时上)(The Weight-Is-Hierarchy Rule)。
