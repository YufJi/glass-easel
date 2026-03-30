# glass-easel 技术实现分析文档

## 1. 概述

glass-easel 是微信小程序的组件化 UI 框架核心实现。它是一个基于组件的声明式 UI 开发框架，设计目标是在不同环境（Web、小程序等）中运行相同的组件代码。

### 1.1 设计哲学

- **跨环境运行**：通过 Backend 抽象层，同一套组件代码可以在浏览器、小程序等不同环境中运行
- **组件化开发**：支持自定义组件、Behavior 混入、模板继承等特性
- **性能优化**：采用 per-component 树更新算法，平衡不同场景的性能表现
- **类型安全**：完整的 TypeScript 支持，提供强大的类型推断

### 1.2 与 Web Components 的关系

glass-easel 在概念上与 Web Components 类似（Shadow DOM、自定义元素），但实现方式不同：

- 不依赖浏览器原生 Web Components API
- 提供自己的组件生命周期和模板系统
- 支持更灵活的后端渲染模式

---

## 2. 架构概览

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                        │
├─────────────────────────────────────────────────────────────┤
│  Component Definition  │  Component Instance                │
│  - Behavior            │  - Data Proxy                      │
│  - Template            │  - ShadowRoot                      │
│  - Properties          │  - Event Target                    │
├─────────────────────────────────────────────────────────────┤
│                   glass-easel Core                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │Component │  │ Element  │  │ShadowRoot│  │Behavior  │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │DataGroup │  │ Template │  │  Event   │  │ClassList │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
├─────────────────────────────────────────────────────────────┤
│                    Backend Abstraction                      │
│     Shadow Mode    │   Composed Mode   │   Domlike Mode    │
├─────────────────────────────────────────────────────────────┤
│                    Environment Layer                        │
│         MiniProgram Runtime    │    Browser DOM             │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心模块

| 模块       | 文件                 | 职责                 |
| ---------- | -------------------- | -------------------- |
| Component  | `src/component.ts`   | 组件实例和组件定义   |
| Element    | `src/element.ts`     | 元素基类，节点树操作 |
| ShadowRoot | `src/shadow_root.ts` | 影子根，插槽管理     |
| Behavior   | `src/behavior.ts`    | 行为系统，组件组合   |
| DataGroup  | `src/data_proxy.ts`  | 数据绑定和观察者     |
| Template   | `src/tmpl/`          | 模板引擎实现         |
| Backend    | `src/backend/`       | 后端抽象协议         |

---

## 3. 核心概念详解

### 3.1 Backend（后端抽象）

Backend 是 glass-easel 的核心抽象，代表框架运行时的当前环境。

#### 3.1.1 Backend 模式

```typescript
export enum BackendMode {
  Shadow = 0, // 原生 Shadow DOM 模式（小程序环境）
  Composed = 1, // 组合模式（虚拟树）
  Domlike = 2, // 类 DOM 模式（浏览器环境）
}
```

**Shadow Mode**：

- 适用于小程序原生环境
- 组件有真正的 Shadow Root
- 样式完全隔离

**Composed Mode**：

- 适用于需要虚拟树的场景
- 手动管理组件树结构
- 灵活的节点操作

**Domlike Mode**：

- 适用于浏览器环境
- 直接操作 DOM API
- 使用标准 DOM 事件

#### 3.1.2 Backend 协议

```typescript
// 后端上下文接口
export interface Context {
  mode: BackendMode.Shadow
  getRootNode(): ShadowRootContext
  createElement(logicalName: string, stylingName: string): Element
  createComponent(...): Element
  // ... 其他方法
}

// 后端元素接口
export interface Element {
  release(): void
  appendChild(child: Element): void
  removeChild(child: Element): void
  setAttribute(name: string, value: unknown): void
  // ... 其他方法
}
```

### 3.2 Component（组件）

#### 3.2.1 组件定义（ComponentDefinition）

```typescript
export class ComponentDefinition<
  TData extends DataList,
  TProperty extends PropertyList,
  TMethod extends MethodList,
> {
  is: string                    // 组件标识名
  behavior: Behavior<...>       // 根行为
  _$detail: ComponentDefinitionDetail | null  // 编译后的细节
  _$options: NormalizedComponentOptions       // 组件选项

  prepare(): void               // 准备阶段（编译优化）
  updateTemplate(template: object): void  // 热更新模板
}
```

#### 3.2.2 组件实例（Component）

```typescript
export class Component<...> extends Element {
  [COMPONENT_SYMBOL]: true
  _$behavior: Behavior<...>     // 行为定义
  _$definition: ComponentDefinition<...>  // 组件定义
  _$dataGroup: DataGroup<...>   // 数据代理
  shadowRoot: ShadowRoot        // 影子根
  templateInstance: TemplateInstance  // 模板实例

  // 生命周期
  triggerLifetime(name: string, args: any[]): void
  addLifetimeListener(name: string, func: Function): void

  // 数据操作
  setData(newData: Partial<...>): void
  replaceDataOnPath(path: DataPath, data: any): void

  // 方法调用
  callMethod<T extends string>(methodName: T, ...args: any[]): any
}
```

#### 3.2.3 组件创建流程

```
ComponentDefinition.prepare()
    ↓
创建原型对象 (proto)
    ↓
初始化 DataGroup
    ↓
创建 TemplateInstance
    ↓
初始化 ShadowRoot
    ↓
触发 created 生命周期
    ↓
应用初始数据到模板
    ↓
触发 attached 生命周期
```

### 3.3 Element（元素）

Element 是所有节点的基类，包括 Component、NativeNode、VirtualNode。

#### 3.3.1 核心属性

```typescript
export class Element implements NodeCast {
  [ELEMENT_SYMBOL]: true
  _$backendElement: GeneralBackendElement | null // 后端元素
  _$nodeTreeContext: GeneralBackendContext | DestroyedBackendContext

  // 树结构
  parentNode: Element | null
  childNodes: Node[]
  parentIndex: number
  ownerShadowRoot: ShadowRoot | null

  // 插槽相关
  _$nodeSlot: string // slot 属性值
  _$slotElement: Element | null // 所在的 slot 元素
  slotNodes: Node[] | undefined // slot 内容节点

  // 样式
  classList: ClassList | null
  _$styleSegments: string[]

  // 事件
  _$eventTarget: EventTarget
}
```

#### 3.3.2 节点操作

```typescript
// 子节点操作
appendChild(child: Node): void
insertBefore(child: Node, ref: Node | null): void
removeChild(child: Node): void
replaceChild(newChild: Node, oldChild: Node): void

// 属性操作
setAttribute(name: string, value: unknown): void
removeAttribute(name: string): void

// 样式操作
setNodeClass(classNames: string, index?: StyleSegmentIndex): void
setNodeStyle(styleSegment: string, index?: StyleSegmentIndex): void
toggleNodeClass(className: string, force?: boolean): void
```

### 3.4 ShadowRoot（影子根）

ShadowRoot 是组件内部的独立 DOM 树，负责插槽管理和内容分发。

#### 3.4.1 插槽模式

```typescript
export enum SlotMode {
  Single = 1, // 单插槽模式
  Multiple, // 多插槽模式（具名插槽）
  Dynamic, // 动态插槽
}
```

**Single Mode**：

- 所有子节点放入单一插槽
- 最简单高效

**Multiple Mode**：

- 支持多个具名插槽
- 通过 `slot` 属性分发内容

**Dynamic Mode**：

- 运行时动态创建插槽
- 支持插槽值的传递和更新

#### 3.4.2 插槽内容分发

```typescript
// 获取指定名称的插槽元素
getSlotElementFromName(name: string): Element | Element[] | null

// 获取节点所在的插槽
getContainingSlot(elem: Node | null): Element | null

// 遍历插槽内容
forEachNodeInSlot(f: (node: Node, slot: Element | null) => boolean | void): boolean
```

### 3.5 Behavior（行为）

Behavior 是 glass-easel 的代码复用机制，类似于 Mixin。

#### 3.5.1 Behavior 结构

```typescript
export class Behavior<...> {
  is: string                    // 行为标识
  ownerSpace: ComponentSpace    // 所属组件空间

  // 数据相关
  _$staticData?: DataList       // 静态数据
  _$data: (() => DataList)[]    // 数据生成函数
  _$propertyMap: { [name: string]: PropertyDefinition }  // 属性定义

  // 方法
  _$methodMap: { [name: string]: ComponentMethod }  // 方法映射

  // 生命周期
  _$lifetimes: { name: string; func: ComponentMethod; once: boolean }[]
  _$pageLifetimes?: { name: string; func: ComponentMethod; once: boolean }[]

  // 观察者
  _$observers: { dataPaths: MultiPaths; observer: ComponentMethod; once: boolean }[]

  // 关系
  _$relationMap?: { [name: string]: RelationDefinition }

  // 初始化函数
  _$init: { func: Function; once: boolean }[]
}
```

#### 3.5.2 Behavior 构建器

```typescript
export class BehaviorBuilder<...> {
  // 链式 API
  data<T extends DataList>(gen: () => T): BehaviorBuilder<...>
  property<N extends string, T extends PropertyType>(
    name: N,
    def: PropertyListItem<T, V>
  ): BehaviorBuilder<...>
  methods<T extends MethodList>(funcs: T): BehaviorBuilder<...>
  observer<P extends string>(
    paths: P,
    func: (newValue: V) => void
  ): BehaviorBuilder<...>
  lifetime<L extends keyof Lifetimes>(
    name: L,
    func: Lifetimes[L]
  ): BehaviorBuilder<...>

  // 注册
  registerBehavior(): Behavior<...>
  registerComponent(): ComponentDefinition<...>
}
```

#### 3.5.3 Behavior 合并（Prepare 阶段）

```
Behavior.prepare()
    ↓
递归准备依赖的 Behavior
    ↓
合并 trait behaviors
    ↓
合并静态数据（shallowMerge）
    ↓
合并方法（Object.assign）
    ↓
合并属性（Object.assign）
    ↓
合并数据生成函数
    ↓
合并观察者（去重）
    ↓
合并生命周期（去重）
    ↓
合并关系定义
    ↓
合并初始化函数
```

---

## 4. 数据管理系统

### 4.1 DataGroup（数据组）

DataGroup 是组件数据的中央管理器，负责数据更新、观察者触发和属性同步。

#### 4.1.1 核心结构

```typescript
export class DataGroup<...> {
  data: DataWithPropertyValues<TData, TProperty>  // 公开数据
  innerData: { [key: string]: DataValue } | null  // 内部数据（深拷贝后）
  updateListener?: DataUpdateCallback              // 更新监听器（模板引擎）

  // 配置
  _$pureDataPattern: RegExp | null        // 纯数据模式（不触发更新）
  _$dataDeepCopy: DeepCopyStrategy        // 数据深拷贝策略
  _$propertyPassingDeepCopy: DeepCopyStrategy  // 属性传递深拷贝策略
  _$reflectToAttributes: boolean          // 是否反射到属性
  _$observerTree: DataGroupObserverTree   // 观察者树
}
```

#### 4.1.2 数据更新流程

```
replaceDataOnPath(path, newData)
    ↓
添加到 pendingChanges 队列
    ↓
applyDataUpdates()
    ↓
遍历 pendingChanges
    ↓
应用数据变更到 this.data
    ↓
更新 innerData（如果需要深拷贝）
    ↓
标记受影响的观察者
    ↓
触发数据观察者
    ↓
调用 updateListener（通知模板引擎）
    ↓
触发属性观察者（property observer）
```

#### 4.1.3 深拷贝策略

```typescript
export enum DeepCopyStrategy {
  None, // 不拷贝，直接引用
  Simple, // 简单深拷贝（一层）
  SimpleWithRecursion, // 递归深拷贝
}
```

### 4.2 观察者系统

#### 4.2.1 观察者树

```typescript
export class DataGroupObserverTree {
  propFields: { [name: string]: PropertyDefinition }
  observerRoot: ObserverNode = { sub: {} }
  observers: DataObserverWithPath[] = []
}

type ObserverNode = {
  listener?: number[] // 监听此路径的观察者索引
  wildcard?: number[] // 通配符 ** 匹配的观察者
  sub: { [name: string]: ObserverNode } // 子路径
}
```

#### 4.2.2 路径匹配

```typescript
// 支持的路径格式
'field' // 监听单个字段
'field.subField' // 监听嵌套字段
'array.0' // 监听数组元素
'field.**' // 监听字段及其所有子字段
'field1,field2' // 监听多个字段（数组形式）
```

### 4.3 属性系统

#### 4.3.1 属性类型

```typescript
export enum NormalizedPropertyType {
  Invalid = 'invalid',
  Any = 'any',
  String = 'string',
  Number = 'number',
  Boolean = 'boolean',
  Object = 'object',
  Array = 'array',
  Function = 'function',
}
```

#### 4.3.2 属性定义

```typescript
export type PropertyDefinition = {
  type: NormalizedPropertyType
  optionalTypes: NormalizedPropertyType[] | null
  defaultFn: () => unknown // 默认值生成函数
  observer: ((newValue: unknown, oldValue: unknown) => void) | null
  comparer: ((newValue: unknown, oldValue: unknown) => boolean) | null
  reflectIdPrefix: boolean
}
```

#### 4.3.3 属性值转换

当属性被赋值时，会自动进行类型转换：

```typescript
export const convertValueToType = (
  value: unknown,
  propName: string,
  prop: PropertyDefinition,
): unknown => {
  // 1. 尝试匹配可选类型
  if (prop.optionalTypes) {
    for (const type of prop.optionalTypes) {
      if (matchTypeWithValue(type, value)) return value
    }
  }

  // 2. 根据主类型转换
  switch (prop.type) {
    case NormalizedPropertyType.String:
      return String(value)
    case NormalizedPropertyType.Number:
      return isFinite(value) ? Number(value) : prop.defaultFn()
    case NormalizedPropertyType.Boolean:
      return !!value
    case NormalizedPropertyType.Array:
      return Array.isArray(value) ? value : prop.defaultFn()
    // ... 其他类型
  }
}
```

---

## 5. 模板系统

### 5.1 架构概述

模板系统采用编译生成代码的方式，将 WXML 模板编译为 JavaScript 函数（ProcGen）。

```
WXML Template
     ↓
Template Compiler (glass-easel-template-compiler, Rust)
     ↓
ProcGen (JavaScript 函数)
     ↓
ProcGenWrapper 执行
     ↓
创建/更新 ShadowRoot 中的节点
```

### 5.2 核心接口

```typescript
// 模板引擎
export interface TemplateEngine {
  create(behavior: GeneralBehavior, options: NormalizedComponentOptions): Template
}

// 编译后的模板
export interface Template {
  createInstance(
    elem: GeneralComponent,
    createShadowRoot: (component: GeneralComponent) => ShadowRoot,
  ): TemplateInstance
}

// 模板实例
export interface TemplateInstance {
  shadowRoot: ShadowRoot | ExternalShadowRoot
  initValues(data: DataValue): void
  updateValues(data: DataValue, changes: DataChange[]): void
}
```

### 5.3 ProcGen（过程生成器）

ProcGen 是模板编译后的核心执行单元。

#### 5.3.1 ProcGen 签名

```typescript
export interface ProcGen {
  // 创建阶段
  (wrapper: ProcGenWrapper, isCreation: true, data: DataValue): {
    C: DefineChildren // 定义子节点
    B?: { [field: string]: BindingMapGen[] } // 绑定映射生成器
  }

  // 更新阶段
  (
    wrapper: ProcGenWrapper,
    isCreation: false,
    data: DataValue,
    dataUpdatePathTree: UpdatePathTreeNode,
  ): { C: DefineChildren }
}
```

#### 5.3.2 DefineChildren

```typescript
export type DefineChildren = (
  isCreation: boolean,
  defineTextNode: DefineTextNode, // 定义文本节点
  defineElement: DefineElement, // 定义元素节点
  defineIfGroup: DefineIfGroup, // 定义条件分支
  defineForLoop: DefineForLoop, // 定义循环
  defineSlot: DefineSlot, // 定义插槽
  definePureVirtualNode: DefinePureVirtualNode, // 定义纯虚拟节点
  dynamicSlotValues: { [name: string]: unknown } | undefined,
  slotValueUpdatePathTrees: UpdatePathTreeNode | undefined,
) => void
```

### 5.4 更新策略

#### 5.4.1 更新模式

```typescript
const enum BindingMapUpdateEnabled {
  Disabled, // 禁用绑定映射更新
  Enabled, // 启用（根据条件自动选择）
  Forced, // 强制使用绑定映射更新
}
```

#### 5.4.2 更新路径树

```typescript
export type UpdatePathTreeNode = true | { [key: string]: UpdatePathTreeNode } | UpdatePathTreeNode[]
```

示例：

```javascript
// 数据变更: { list: [1, 2, 3], count: 5 }
// 更新路径树:
{
  list: true,      // list 字段完全更新
  count: true      // count 字段更新
}

// 数组 splice 变更
{
  items: Object.create([0, 0, true, true])  // 原型链上的数组表示 splice 操作
}
```

#### 5.4.3 更新流程

```
updateValues(data, changes)
    ↓
构建 dataUpdatePathTree
    ↓
if (forceBindingMapUpdate) {
    尝试使用 bindingMapUpdate（字段级精确更新）
} else {
    调用 procGen 生成更新后的子树
}
    ↓
对比新旧子树差异
    ↓
应用最小化 DOM 更新
```

### 5.5 列表差异算法

glass-easel 使用自定义的列表差异算法（在 `range_list_diff.ts` 中实现），优化 `wx:for` 的更新性能。

---

## 6. 样式系统

### 6.1 StyleScope（样式作用域）

#### 6.1.1 作用域管理

```typescript
export type StyleScopeId = number

export class StyleScopeManager {
  private _$names: string[] = ['']

  register(name: string): StyleScopeId {
    const ret = this._$names.length
    this._$names.push(name)
    return ret
  }

  queryName(id: StyleScopeId): string | undefined {
    return this._$names[id]
  }

  static globalScope(): StyleScopeId {
    return 0
  }
}
```

#### 6.1.2 类名解析

```typescript
// 前缀约定
'classname' // 默认作用域
'~classname' // 根作用域
'^classname' // 父级作用域
'^^classname' // 祖父级作用域
```

### 6.2 ClassList

ClassList 管理元素的类名，支持多段（segment）和外部类。

#### 6.2.1 样式段（Style Segment）

```typescript
export enum StyleSegmentIndex {
  MAIN = 0, // 主样式段（模板引擎管理）
  TEMPLATE_EXTRA = 1, // 模板额外样式
  ANIMATION_EXTRA = 2, // 动画样式
  TEMP_EXTRA = 3, // 临时高优先级样式
}
```

#### 6.2.2 外部类（External Classes）

外部类允许父组件影响子组件的样式：

```typescript
export class ClassList {
  _$externalNames: string[] | undefined // 外部类名列表
  _$externalRawAlias: (string[] | undefined)[] // 外部类映射
  _$owner: ClassList | null // 父级 ClassList

  // 设置外部类别名
  _$setAlias(name: string, target: string | string[]): void

  // 解析前缀（~, ^ 等）
  private _$resolvePrefixes(
    name: string,
    cb: (scopeId: StyleScopeId | undefined, className: string) => void,
  ): void
}
```

### 6.3 虚拟宿主（Virtual Host）

Virtual Host 是一种特殊的组件模式，组件不创建自己的宿主元素，而是直接渲染子内容。

```typescript
// 在组件选项中启用
options: {
  virtualHost: true
}

// 效果：
// - 不创建 backendElement
// - 样式直接应用到组件内容
// - 减少一层 DOM 嵌套
```

---

## 7. 事件系统

### 7.1 事件结构

```typescript
export class Event<TDetail> {
  type: string
  timeStamp: number
  detail: TDetail
  bubbles: boolean
  composed: boolean
  eventPhase: EventPhase

  // 冒泡控制
  preventDefault(): void
  stopPropagation(): void
  markMutated(): void
}

export enum EventPhase {
  None,
  CapturingPhase, // 捕获阶段
  AtTarget, // 目标阶段
  BubblingPhase, // 冒泡阶段
}
```

### 7.2 事件目标

```typescript
export class EventTarget<TEvents extends { [type: string]: unknown }> {
  listeners: { [T in keyof TEvents]: EventFuncArr<TEvents[T]> }
  captureListeners: { [T in keyof TEvents]: EventFuncArr<TEvents[T]> } | null

  addListener<T extends string>(
    name: T,
    func: EventListener<TEvents[T]>,
    options?: EventListenerOptions,
  ): FinalChanged

  removeListener<T extends string>(
    name: T,
    func: EventListener<TEvents[T]>,
    options?: EventListenerOptions,
  ): FinalChanged
}
```

### 7.3 事件分发流程

```
event.dispatch(target)
    ↓
构建冒泡路径（考虑跨 ShadowRoot）
    ↓
if (capturePhase) {
    从根到目标执行捕获监听器
}
    ↓
目标阶段（At Target）
    ↓
if (bubbles && !stopped) {
    从目标到根执行冒泡监听器
}
```

### 7.4 跨 ShadowRoot 冒泡

```typescript
const forEachBubblePath = (target: Element, f: Function) => {
  const recShadow = (target: Element): Element | null => {
    let currentTarget = target
    for (;;) {
      f(currentTarget, target, mark)

      if (crossShadow) {
        // 跨 ShadowRoot
        if (isShadowRoot(currentTarget)) {
          return currentTarget.getHostNode() // 跳到宿主组件
        }
        if (currentTarget.containingSlot) {
          return recShadow(currentTarget.containingSlot) // 递归到 slot
        }
      }

      currentTarget = currentTarget.parentNode
      if (!currentTarget) return null
    }
  }
  // ...
}
```

---

## 8. 组件空间（ComponentSpace）

### 8.1 作用

ComponentSpace 是组件的命名空间，管理组件注册、查找和依赖关系。

### 8.2 核心功能

```typescript
export class ComponentSpace {
  // 组件注册表
  private _$list: { [path: string]: GeneralComponentDefinition }
  private _$pubList: { [path: string]: GeneralComponentDefinition }

  // 行为注册表
  private _$behaviorList: { [path: string]: GeneralBehavior }
  private _$pubBehaviorList: { [path: string]: GeneralBehavior }

  // 全局 using 组件
  private _$using: { [path: string]: GeneralComponentDefinition | NativeNodeDefinition }

  // 等待列表（用于延迟加载）
  private _$listWaiting: { [path: string]: ComponentWaitingList }

  // 样式作用域管理器
  readonly styleScopeManager: StyleScopeManager

  // 中间件钩子
  readonly hooks: ComponentSpaceHooks
}
```

### 8.3 组件解析流程

```
resolveComponent(tagName)
    ↓
1. 查找 using 列表
    ↓
2. 查找 generics 实现
    ↓
3. 查找全局 using
    ↓
4. 使用默认组件
```

---

## 9. 生命周期

### 9.1 组件生命周期

```typescript
export type Lifetimes = {
  created: () => void           // 组件实例创建完成
  attached: () => void          // 组件被添加到节点树
  ready: () => void             // 组件渲染完成
  moved: () => void             // 组件在节点树中移动
  detached: () => void          // 组件从节点树移除
  error: (err: unknown) => void // 组件发生错误
  listenerChange: (...) => void // 监听器变化
  workletChange: (...) => void  // Worklet 变化
}
```

### 9.2 生命周期触发时机

```
Component._$advancedCreate()
    ↓
创建 backendElement
    ↓
创建 ShadowRoot
    ↓
创建 TemplateInstance
    ↓
初始化 DataGroup
    ↓
triggerLifetime('created')
    ↓
（父节点添加子节点）
    ↓
Element.checkAndCallAttached()
    ↓
triggerLifetime('attached')
    ↓
（首次渲染完成）
    ↓
triggerLifetime('ready')
```

### 9.3 页面生命周期

```typescript
export type PageLifetimeFuncs = {
  [name: string]: FuncArr<GeneralFuncType>
}

// 页面生命周期通过递归遍历组件树触发
triggerPageLifetime(name: string, args: Parameters<GeneralFuncType>) {
  const rec = (node: Element) => {
    if (isComponent(node)) {
      // 触发当前组件的页面生命周期
      const f = node._$pageLifetimeFuncs[name]
      if (f) f.call(node._$methodCaller, args, this)

      // 递归到 ShadowRoot
      if (!node._$external) rec(node.shadowRoot as ShadowRoot)
    }
    // 递归到子节点
    node.childNodes.forEach(rec)
  }
  rec(this)
}
```

---

## 10. 关系系统（Relations）

### 10.1 关系定义

```typescript
export type RelationDefinition = {
  type: RelationType
  target: string | GeneralBehavior | TraitBehavior<any, any>
}

export enum RelationType {
  ParentNonVirtualNode = 'parent',
  ParentComponent = 'parentComponent',
  Ancestor = 'ancestor',
  Child = 'child',
  Descendant = 'descendant',
}
```

### 10.2 关系触发

关系在组件 attached/detached 时自动建立和断开：

```typescript
// attached 时
if (node._$relation) {
  node._$relation.triggerLinkEvent(RelationType.ParentNonVirtualNode, false)
  node._$relation.triggerLinkEvent(RelationType.ParentComponent, false)
  node._$relation.triggerLinkEvent(RelationType.Ancestor, false)
}

// detached 时
if (node._$relation) {
  node._$relation.triggerLinkEvent(RelationType.ParentNonVirtualNode, true)
  node._$relation.triggerLinkEvent(RelationType.ParentComponent, true)
  node._$relation.triggerLinkEvent(RelationType.Ancestor, true)
}
```

---

## 11. 性能优化

### 11.1 准备阶段（Prepare Phase）

Behavior 的 `prepare()` 方法在组件首次创建时执行，将定义转换为高效的运行时结构：

- 合并所有依赖的 Behavior
- 构建观察者树
- 预处理生命周期函数
- 建立方法映射

### 11.2 批量更新

数据更新采用批量处理模式：

```typescript
replaceDataOnPath(path, data) // 只是加入队列
applyDataUpdates() // 批量应用
```

### 11.3 绑定映射更新（Binding Map Update）

对于简单字段更新，使用绑定映射直接更新，避免重新渲染整个子树。

### 11.4 虚拟节点

VirtualNode 不创建后端元素，减少 DOM 操作开销。

---

## 12. 总结

glass-easel 是一个设计精良的组件化框架，其核心特点包括：

1. **Backend 抽象**：通过统一的 Backend 接口，实现跨环境运行
2. **Behavior 系统**：灵活的代码复用机制
3. **编译优化**：模板预编译为高效的过程代码
4. **细粒度更新**：数据变化精确到字段级别
5. **类型安全**：完整的 TypeScript 类型支持

框架的代码组织清晰，各模块职责分明：

- `component.ts`：组件核心
- `element.ts`：节点树操作
- `shadow_root.ts`：影子根和插槽
- `behavior.ts`：行为系统
- `data_proxy.ts`：数据绑定
- `tmpl/`：模板引擎
- `backend/`：后端协议

这种架构使得 glass-easel 既能满足小程序环境的特殊需求，又能灵活适配其他运行环境。
