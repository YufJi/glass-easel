# glass-easel-template-compiler 技术实现分析

## 1. 项目概述

**glass-easel-template-compiler** 是微信小程序框架 glass-easel 的模板编译器，使用 **Rust** 编写，用于将 WXML 模板编译成 JavaScript 代码。该编译器支持 WebAssembly 输出，可在浏览器和 Node.js 环境中运行。

### 核心特性

- 将 WXML 模板编译为可执行的 JavaScript 代码
- 支持微信小程序的模板语法（wx:for, wx:if, 数据绑定等）
- 提供 TypeScript 类型检查支持
- 生成 SourceMap 用于调试
- 支持代码压缩和变量混淆

---

## 2. 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Template Compilation Pipeline              │
├─────────────────────────────────────────────────────────────┤
│  Input (WXML) → Parse → AST → ProcGen → Stringify → Output  │
│                                                              │
│  ┌─────────┐   ┌─────┐   ┌────────┐   ┌──────────┐          │
│  │  parse  │ → │ AST │ → │proc_gen│ → │ stringify│ → JS     │
│  │ module  │   │     │   │ module │   │  module  │          │
│  └─────────┘   └─────┘   └────────┘   └──────────┘          │
└─────────────────────────────────────────────────────────────┘
```

### 模块结构

| 模块          | 文件                                                                           | 职责                                |
| ------------- | ------------------------------------------------------------------------------ | ----------------------------------- |
| `parse`       | `mod.rs`, `tag.rs`, `expr.rs`, `iter.rs`                                       | 解析 WXML 模板，生成 AST            |
| `proc_gen`    | `mod.rs`, `tag.rs`, `expr.rs`                                                  | 将 AST 转换为过程式 JavaScript 代码 |
| `stringify`   | `mod.rs`, `stringifier.rs`, `tag.rs`, `expr.rs`, `typescript.rs`, `options.rs` | 将 AST 序列化为 WXML 字符串         |
| `group`       | `group.rs`                                                                     | 管理模板组和运行时环境              |
| `binding_map` | `binding_map.rs`                                                               | 数据绑定映射管理                    |

---

## 3. 解析层 (Parse)

### 3.1 核心数据结构

#### Template (模板根节点)

```rust
pub struct Template {
    pub path: String,                    // 模板路径
    pub content: Vec<Node>,              // 子节点列表
    pub globals: TemplateGlobals,        // 全局定义（imports, scripts等）
}
```

#### Node (节点枚举)

```rust
pub enum Node {
    Text(Value),           // 文本节点
    Element(Element),      // 元素节点
    Comment(Comment),      // 注释节点
    UnknownMetaTag(...),   // 未知元标签
}
```

#### ElementKind (元素类型)

```rust
pub enum ElementKind {
    Normal { ... },        // 普通元素
    Pure { ... },          // 纯容器（block）
    For { ... },           // wx:for 循环
    If { ... },            // wx:if 条件
    TemplateRef { ... },   // 模板引用
    Include { ... },       // 文件包含
    Slot { ... },          // 插槽
}
```

### 3.2 表达式解析

表达式解析器实现了完整的 JavaScript 表达式语法支持：

```rust
pub enum Expression {
    // 作用域引用
    ScopeRef { index: usize, ... },
    DataField { name: CompactString, ... },

    // 字面量
    LitUndefined, LitNull, LitStr { ... },
    LitInt { value: i64 }, LitFloat { value: f64 },
    LitBool { value: bool },
    LitObj { fields: Vec<ObjectFieldKind> },
    LitArr { fields: Vec<ArrayFieldKind> },

    // 成员访问和调用
    StaticMember { obj, field_name, ... },
    DynamicMember { obj, field_name, ... },
    FuncCall { func, args, ... },

    // 运算符（完整支持 JS 运算符优先级）
    Reverse, BitReverse, Positive, Negative, TypeOf, Void,
    Multiply, Divide, Remainer, Plus, Minus,
    LeftShift, RightShift, UnsignedRightShift,
    Lt, Gt, Lte, Gte, InstanceOf,
    Eq, Ne, EqFull, NeFull,
    BitAnd, BitXor, BitOr,
    LogicAnd, LogicOr, NullishCoalescing,
    Cond { cond, true_br, false_br },
}
```

### 3.3 解析技术

#### 递归下降解析

使用递归下降算法解析表达式，遵循 JavaScript 运算符优先级：

```rust
// 运算符优先级（从高到低）
parse_reverse → parse_member → parse_multiply → parse_plus →
parse_shift → parse_cmp → parse_eq → parse_bit_and →
parse_bit_xor → parse_bit_or → parse_logic_and →
parse_logic_or → parse_cond
```

#### 状态管理

`ParseState` 管理解析过程中的状态：

- 位置追踪（行号、列号）
- 错误收集
- 回溯支持 (`try_parse`)
- 空白字符处理

---

## 4. 代码生成层 (ProcGen)

### 4.1 设计目标

将 AST 转换为高效的 JavaScript 代码，用于运行时渲染。生成的代码采用**过程式**风格，直接操作虚拟 DOM。

### 4.2 JavaScript 代码生成器

使用自定义的 JavaScript 代码写入器，支持作用域管理：

```rust
pub struct JsTopScopeWriter<W: fmt::Write> {
    w: W,
    top_declares: Vec<String>,    // 顶层变量声明
    sub_strs: Vec<String>,        // 子语句
    block: JsBlockStat,
}

pub struct JsFunctionScopeWriter<'a, W: fmt::Write> {
    w: &'a mut String,
    block: Option<&'a mut JsBlockStat>,
    top_scope: &'a mut JsTopScopeWriter<W>,
}
```

### 4.3 保留变量命名

编译器使用单字母变量名，每个都有特定含义：

| 变量 | 含义                             |
| ---- | -------------------------------- |
| `A`  | 绑定映射对象 (binding map)       |
| `C`  | 是否为创建阶段                   |
| `D`  | 数据值 / 脚本运行时              |
| `E`  | DefineElement 函数               |
| `F`  | DefineForLoop 函数               |
| `G`  | 模板组列表                       |
| `H`  | 当前模板组                       |
| `I`  | 导入的组                         |
| `J`  | DefinePureVirtualNode 函数       |
| `K`  | 整个数据是否改变                 |
| `N`  | 操作的节点                       |
| `R`  | ProcGenWrapper 对象              |
| `T`  | DefineTextNode / updateText 函数 |
| `U`  | 更新路径树                       |
| `V`  | 当前插槽值                       |
| `W`  | 插槽值的更新路径树               |

### 4.4 代码生成示例

对于模板：

```html
<view class="{{className}}">{{message}}</view>
```

生成类似以下的 JavaScript：

```javascript
E(
  'view',
  {},
  function (N, C) {
    // class 绑定
    R.e(N, [C || K ? className : null])
    // 文本节点
    C || K ? T(Y(message)) : T()
  },
  childFunc,
)
```

---

## 5. 序列化层 (Stringify)

### 5.1 功能

将 AST 序列化回 WXML 字符串，支持：

- 代码格式化（缩进、换行）
- 代码压缩（minimize）
- SourceMap 生成
- TypeScript 类型生成

### 5.2 Stringifier 结构

```rust
pub struct Stringifier<'s, W: FmtWrite> {
    w: W,                          // 输出写入器
    line: u32,                     // 当前行
    utf16_col: u32,                // 当前列（UTF-16）
    smb: Option<SourceMapBuilder>, // SourceMap 构建器
    source_path: &'s str,          // 源文件路径
    options: StringifyOptions,     // 配置选项
}
```

### 5.3 TypeScript 支持

`typescript.rs` 模块提供 TypeScript 类型生成，用于：

- 模板表达式的类型检查
- 组件属性的类型推断
- 数据绑定的类型安全

---

## 6. 模板组 (TmplGroup)

### 6.1 功能

管理多个相关模板，支持：

- 模板间的相互引用
- 脚本模块管理
- 运行时环境生成

### 6.2 运行时环境

生成的 JavaScript 包含运行时辅助函数：

```javascript
// 运行时辅助函数
var X = function (a) {
  return a == null ? Object.create(null) : a
}
var Y = function (a) {
  return a == null ? '' : String(a)
}
var Z = function (a, b) {
  if (a === true) return true
  if (a) return a[b]
}
```

### 6.3 WXS 脚本支持

支持内联和外部 WXS 脚本，使用 CommonJS 模块系统：

```javascript
var D = (() => {
    var modules = Object.create(null);
    var load = (filename) => { ... };
    return (filename, func) => { ... };
})()
```

---

## 7. 数据绑定映射 (BindingMap)

### 7.1 作用

追踪模板中使用的数据字段，用于：

- 精确的数据变更检测
- 性能优化（只更新变化的字段）

### 7.2 实现

```rust
pub struct BindingMapCollector {
    overall_disabled: bool,
    fields: HashMap<String, BindingMapField>,
}

pub enum BindingMapField {
    Mapped(usize),    // 字段被映射，记录索引
    Disabled,         // 字段被禁用
}
```

---

## 8. 关键技术亮点

### 8.1 作用域管理

编译器实现了完善的作用域系统：

- **Script Scope**: WXS 脚本模块引入的作用域
- **For Scope**: `wx:for` 引入的 `item` 和 `index`
- **Slot Scope**: 插槽值引入的作用域

作用域索引规则：越近的作用域索引越大，脚本模块作用域索引最小。

### 8.2 增量更新优化

生成的代码包含精细的更新控制：

- `C` 标志：是否为创建阶段
- `K` 标志：整个数据是否改变
- `U` 更新路径树：追踪具体变更路径

### 8.3 错误处理

完善的错误系统：

- 4 个错误级别：Note, Warn, Error, Fatal
- 精确的错误位置（行号、列号）
- 30+ 种错误类型

### 8.4 性能优化

- **紧凑字符串**: 使用 `compact_str` 减少内存分配
- **变量名压缩**: 自动生成短变量名
- **代码压缩**: 支持去除空白和注释

---

## 9. 总结

glass-easel-template-compiler 是一个设计精良的模板编译器，具有以下特点：

1. **清晰的架构**: 解析 → AST → 代码生成 → 序列化，职责分明
2. **完整的语法支持**: 支持微信小程序的所有模板语法
3. **高性能**: Rust 实现 + 精细的内存管理
4. **类型安全**: TypeScript 支持 + SourceMap
5. **可扩展**: 模块化的设计，易于添加新特性

该编译器成功地将声明式的 WXML 模板转换为高效的过程式 JavaScript 代码，为 glass-easel 框架提供了强大的模板处理能力。
