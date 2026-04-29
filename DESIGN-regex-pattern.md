# Design: Pattern 正则表达式重设计

## 目标

将 pattern 模块从"构造化"正则转换为标准正则语义，同时通过 HIR 分析实现不可能模式早退和全匹配分流。

## 变更概览

| 维度 | 旧设计 | 新设计 |
|------|--------|--------|
| `.` 语义 | `[a-zA-Z0-9_]`（标识符字符） | `.`（标准正则：任意字符） |
| Fuzzy 触发 | 仅 `. * \| ( )` | 所有正则元字符：`. * + ? \| ( ) [ ] { } \ ^ $` |
| `.*` 等宽泛模式 | 拒绝（最少 2 字面字符） | 允许，走正常 Fuzzy 或 All 分流 |
| `\w` `\d` `+` `?` | 不识别 → Exact 模式（字面匹配） | 标准 Fuzzy 正则 |
| Scope `\` 分隔符 | `\` + 字母 | `\\`（双反斜杠） |
| 早退判定 | 无 | HIR 分析：字符类与 `\w` 求交判空 |
| 全匹配分流 | 仅 `...` | `.*` `.+` `\w+` `\w*` 自动进入 All 路径 |
| 正则编译 | `Regex::new` | `Regex::new`（HIR 只做分析，不修改 pattern） |

## 核心设计：HIR 只读分析

```
用户 pattern string
        │
        ▼
  regex-syntax 解析 → HIR
        │
        ▼
  analyze(hir)  ← 只读递归遍历
        │
   ┌────┼──────────┐
   │    │          │
Impossible  MatchesAll  Normal
   │    │          │
   ▼    ▼          ▼
  早退  All 路径  Regex::new("^(?:pattern)$")
                   标准编译，全部引擎优化保留
```

**关键决策：HIR 不做变换。** 实验验证（`.vibewire/experiments/hir-regex-compile.md`）表明：
- `.` 交 `\w` 后产生 Unicode 膨胀（数百 range），导致匹配慢 37x
- 所有字符类要么交集后不变（`\d`、`[a-z]`），要么保留原样性能更优（`.`、`\D`、`\S`）
- 在 `\w+` 输入上，原始类和交集类的匹配行为完全一致
- 交集的真正价值只有判空（检测不可能模式），而非替换

## 分析规则

### 判空检测（Early Termination）

定义名 = 非空 `\w+` 字符串。对 HIR 递归遍历，检测 pattern 是否不可能匹配任何 `\w+` 字符串。

**字符类**：`class.intersect(&\w_class)`，交集为空 → Impossible

**字面量**：含非 `\w` 字节 → Impossible（字面量要求精确匹配）

**重复** `Repetition { min, sub }`：
- sub 为 Impossible 且 min > 0 → Impossible（如 `\s+`）
- sub 为 Impossible 且 min == 0 → 不阻止（可匹配零次，如 `\s*`）

**连接** `Concat`：任一必需元素 Impossible → 整体 Impossible

**选择** `Alternation`：Impossible 分支静默丢弃；全部 Impossible → 整体 Impossible

**零宽断言** `Concat` 中位置感知：

| 断言 | 首位/末位 | 中间位置 |
|------|----------|---------|
| `\b` | 不阻止（全锚定已提供边界） | Impossible（`\w+` 内部无词边界） |
| `\B` | Impossible（首尾存在词边界） | 不阻止（`\w+` 内部始终无词边界） |
| `^` `$` | 不阻止（全锚定冗余） | 不阻止 |

### 全匹配分流

分析后 HIR 顶层是否为"匹配所有 `\w+` 字符串"的模式：

- `.*` / `.+` / `\w+` / `\w*` → All 模式（跳过 screening，matches_ident 全通过）
- Group 包裹的上述模式同理：`(.*)` / `(?:\w+)` 等

判定方式：检查顶层是否为覆盖所有 `\w` 字符的字符类的 `*`/`+` 重复。

## 编译路径

分析通过后（非 Impossible、非 All），pattern 字符串原样传入两条编译路径：

**matches_ident**（编译一次，全生命周期复用）：
```
compile_anchored_regex(pattern, case_insensitive)
→ "^(?:pattern)$"  →  Regex::new()
```

**screening**（每文件由 parser 构建前缀）：
```
parser.regex_pattern(screening_pattern, kinds)
→ "prefix(?:pattern)\b"  →  Regex::new()
```

两条路径都使用标准 `Regex::new()`，保留 regex 引擎的全部优化（prefilter、多引擎组合等）。

## Scope 分隔符变更

| 分隔符 | 旧语法 | 新语法 | 影响语言 |
|--------|--------|--------|---------|
| `::` | `MyClass::method` | 不变 | Rust, C++, Ruby, PHP |
| `.` | `MyClass\.method` | 不变 | Java, Go, Python, JS/TS, C#, Kotlin |
| `\` | `App\Models\User` | `App\\Models\\User` | PHP 命名空间 |

变更原因：`\w`、`\d` 等正则转义与旧的单反斜杠 scope 检测冲突。双反斜杠消除了歧义。

## 依赖变更

```toml
[dependencies]
regex = "1"           # 保留（标准编译）
regex-syntax = "0.8"  # 新增（HIR 解析 + 分析）
```

不需要 `regex-automata`（不做 `build_from_hir`）。

## 接口变更

### `MatchMode` 变体

```rust
pub enum MatchMode {
    Exact { name: String, case_insensitive: bool },
    Fuzzy {
        original: String,    // 用户原始输入
        compiled: Regex,     // ^(?:original)$
        case_insensitive: bool,
    },
    All,
}
```

移除 `Fuzzy.regex_str`：旧设计中 `regex_str` 存储变换后的正则（`replace_dots` 结果），新设计中 pattern 不做变换，`original` 即为编译使用的字符串。

### `screening_pattern()` 返回值

- Exact → `regex::escape(name)`（不变）
- Fuzzy → `original`（原样返回，不做任何变换）
- All → panic（不变）

### `is_fuzzy_char()` 扩展

```rust
fn is_regex_meta(c: char) -> bool {
    matches!(c, '.' | '*' | '+' | '?' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '^' | '$')
}
```

### 移除的函数

- `replace_dots()` — `.` 不再替换为 `[a-zA-Z0-9_]`
- `max_literal_segment()` — 不再限制"过于宽泛"
- `IDENT_CHAR_CLASS` 常量 — 不再需要

### Scope 检测变更

`detect_scope_separator` Priority 3 从 `\` + ASCII letter 改为 `\\`（连续两个反斜杠）。

## 破坏性变更

| 旧行为 | 新行为 | 影响 |
|--------|--------|------|
| `My.*ss` → `My[a-zA-Z0-9_]*ss` | `My.*ss` → `My.*ss`（标准正则） | `.` 匹配任意字符而非仅标识符字符 |
| `.*` → 拒绝 | `.*` → All 模式 | 不再拒绝宽泛模式 |
| `a.*` → 拒绝 | `a.*` → Fuzzy 正常编译 | 同上 |
| `\w+` → Exact（字面匹配 `\w+`） | `\w+` → All 模式 | 标准正则语义 |
| `App\Models` → scope 检测 | `App\Models` → scope 检测 | **不变**（单 `\` + 字母仍触发） |
| `App\\Models` → 无 scope | `App\\Models` → scope 检测 | **新增**（双反斜杠触发） |

注意：`\w` 在旧设计中走 Exact 模式（按字面匹配 `\w` 两个字符），新设计中走标准 Fuzzy 正则。这是预期的行为变更。

## 实验依据

- `.vibewire/experiments/hir-regex-compile.md`：HIR 编译路径验证
- `.vibewire/tech-research/BUILD-regex-pattern-redesign.md`：ripgrep/grep 正则处理调研
