import { type GeneralComponent, type Component } from './component'
import { Element } from './element'
import { type NativeNode } from './native_node'
import { type Node } from './node'
import { type ShadowRoot } from './shadow_root'
import { type TextNode } from './text_node'
import { isElement, isTextNode } from './type_symbol'
import { type VirtualNode } from './virtual_node'

/** The iterator direction and order */
export const enum ElementIteratorType {
  /** Iterate all ancestors in shadow tree */
  ShadowAncestors = 'shadow-ancestors',
  /** Iterate all ancestors in composed tree */
  ComposedAncestors = 'composed-ancestors',
  /** Iterate all descendants in shadow tree, returning parents before their children */
  ShadowDescendantsRootFirst = 'shadow-descendants-root-first',
  /** Iterate all descendants in shadow tree, returning parents after their children */
  ShadowDescendantsRootLast = 'shadow-descendants-root-last',
  /** Iterate all descendants in composed tree, returning parents before their children */
  ComposedDescendantsRootFirst = 'composed-descendants-root-first',
  /** Iterate all descendants in composed tree, returning parents after their children */
  ComposedDescendantsRootLast = 'composed-descendants-root-last',
}

/**
 * An iterator for node tree traversal
 *
 * This iterator is convenient but seems a little slower.
 */
export class ElementIterator<T extends Node = Element> {
  /* @internal */
  private _$node: Node
  /* @internal */
  private _$nodeTypeLimit: unknown
  /* @internal */
  private _$composed: boolean
  /* @internal */
  private _$isAncestor: boolean
  /* @internal */
  private _$rootFirst: boolean

  /**
   * Create an iterator with type specified
   *
   * The `nodeTypeLimit` is used to limit which kind of nodes will be returned.
   * It limits the returned result by an `instanceof` call.
   * The default value is `Element` ,
   * which means only elements will be returned (text nodes will not).
   * Consider specifying `Object` if text nodes need to be returned as well as elements.
   * Specify `Component` will only return components.
   */
  constructor(node: Node, type: ElementIteratorType, nodeTypeLimit: unknown = Element) {
    if (!isElement(node) && !isTextNode(node)) {
      throw new Error('Element iterators can only be used in elements or text nodes')
    }
    this._$node = node
    this._$nodeTypeLimit = nodeTypeLimit || Element
    if (
      type === ElementIteratorType.ShadowAncestors ||
      type === ElementIteratorType.ShadowDescendantsRootFirst ||
      type === ElementIteratorType.ShadowDescendantsRootLast
    ) {
      this._$composed = false
    } else if (
      type === ElementIteratorType.ComposedAncestors ||
      type === ElementIteratorType.ComposedDescendantsRootFirst ||
      type === ElementIteratorType.ComposedDescendantsRootLast
    ) {
      this._$composed = true
    } else {
      throw new Error(`Unrecognized iterator type "${String(type)}"`)
    }
    if (
      type === ElementIteratorType.ShadowAncestors ||
      type === ElementIteratorType.ComposedAncestors
    ) {
      this._$isAncestor = true
    } else {
      this._$isAncestor = false
    }
    if (
      type === ElementIteratorType.ShadowDescendantsRootFirst ||
      type === ElementIteratorType.ComposedDescendantsRootFirst
    ) {
      this._$rootFirst = true
    } else {
      this._$rootFirst = false
    }
  }

  /** Same as constructor (for backward compatibility) */
  static create(
    node: Node,
    type: ElementIteratorType,
    nodeTypeLimit?: typeof Element,
  ): ElementIterator<Element>
  static create(
    node: Node,
    type: ElementIteratorType,
    nodeTypeLimit: typeof Component,
  ): ElementIterator<GeneralComponent>
  static create(
    node: Node,
    type: ElementIteratorType,
    nodeTypeLimit: typeof NativeNode,
  ): ElementIterator<NativeNode>
  static create(
    node: Node,
    type: ElementIteratorType,
    nodeTypeLimit: typeof ShadowRoot,
  ): ElementIterator<ShadowRoot>
  static create(
    node: Node,
    type: ElementIteratorType,
    nodeTypeLimit: typeof VirtualNode,
  ): ElementIterator<VirtualNode>
  static create(
    node: Node,
    type: ElementIteratorType,
    nodeTypeLimit: typeof TextNode,
  ): ElementIterator<TextNode>
  static create(
    node: Node,
    type: ElementIteratorType,
    nodeTypeLimit: typeof Object,
  ): ElementIterator<Node>
  static create(node: Node, type: ElementIteratorType, nodeTypeLimit?: unknown) {
    return new ElementIterator<Node>(node, type, nodeTypeLimit)
  }

  [Symbol.iterator](): Generator<T, void, boolean | void> {
    return this._$getIterator()
  }

  private *_$getIterator(): Generator<T, void, boolean | void> {
    const nodeTypeLimit: any = this._$nodeTypeLimit
    const composed = this._$composed
    if (this._$isAncestor) {
      let cur = this._$node
      for (;;) {
        if (cur instanceof nodeTypeLimit) {
          if ((yield cur as T) === false) return
        }
        let next: Element | null
        if (composed) {
          next = cur.getComposedParent()
        } else {
          next = cur.parentNode
        }
        if (next) cur = next
        else break
      }
    } else {
      const rootFirst = this._$rootFirst
      const rec = function* (node: Node): Generator<T, void, boolean | void> {
        if (rootFirst) {
          if (node instanceof nodeTypeLimit) {
            if ((yield node as T) === false) return
          }
        }
        if (isElement(node)) {
          if (composed) {
            const iterator = node.iterateComposedChild()
            for (let it = iterator.next(); !it.done; it = iterator.next()) {
              yield* rec(it.value)
            }
          } else {
            const childNodes = node.childNodes
            for (let i = 0; i < childNodes.length; i += 1) {
              const child = childNodes[i]!
              yield* rec(child)
            }
          }
        }
        if (!rootFirst) {
          if (node instanceof nodeTypeLimit) {
            yield node as T
          }
        }
      }
      yield* rec(this._$node)
    }
  }

  forEach(f: (node: T) => boolean | void) {
    const nodeTypeLimit: any = this._$nodeTypeLimit
    const composed = this._$composed
    if (this._$isAncestor) {
      let cur = this._$node
      for (;;) {
        if (cur instanceof nodeTypeLimit) {
          if (f(cur as T) === false) return
        }
        let next: Element | null
        if (composed) {
          next = cur.getComposedParent()
        } else {
          next = cur.parentNode
        }
        if (next) cur = next
        else break
      }
    } else {
      const rootFirst = this._$rootFirst
      const rec = (node: Node): boolean => {
        if (rootFirst) {
          if (node instanceof nodeTypeLimit) {
            if (f(node as T) === false) return false
          }
        }
        if (isElement(node)) {
          let interrupted = false
          const childFn = (child: Node) => {
            if (rec(child) === false) {
              interrupted = true
              return false
            }
            return true
          }
          if (composed) {
            node.forEachComposedChild(childFn)
          } else {
            node.childNodes.every(childFn)
          }
          if (interrupted) return false
        }
        if (!rootFirst) {
          if (node instanceof nodeTypeLimit) {
            if (f(node as T) === false) return false
          }
        }
        return true
      }
      rec(this._$node)
    }
  }
}
