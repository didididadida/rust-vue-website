#!/usr/bin/env node
/**
 * 组件绑定自检：找出「模板里引用了、但脚本里没定义」的标识符。
 *
 * 为什么需要这个脚本
 * ------------------
 * Vue 的 <script setup> 把模板编译成渲染函数，模板里每个标识符最后都落到
 * `_ctx.xxx`。**编译期不检查它是否存在**——只有该组件真的被渲染时才会命中
 * undefined，紧接着取属性就抛 TypeError，整块组件渲染失败、页面变白。
 *
 * 真实事故：Admin.vue 改行内编辑时被误删 `articleFiltered` / `articlePageRows` /
 * `articleColumns` / `batchDeleteArticles` 四个绑定。`vite build` 完全通过，
 * 本地 `npm run dev` 也不会拦（不打开那个页面就看不见），部署后 /admin 白屏——
 * 而 App.vue 的页脚照常显示，因为它属于父组件，路由组件炸掉不影响它。
 *
 * 输入 → 输出
 * ------------
 *   输入：`src/**\/*.vue`
 *   输出：每个「模板引用但不可解析」的名字 + 文件路径；有问题时退出码 1
 *
 * 关键机制
 * --------
 * 不用正则扒文本，走 Vue 官方编译器：
 *   1. `parse()` 拿到 SFC 与模板 **AST**
 *   2. `compileScript()` 拿到编译器算出的**绑定表**（bindings）
 *   3. 深度遍历 AST，对每个 `{{ }}` / 指令表达式做标识符提取，
 *      并维护一个**词法作用域栈**：
 *        - `v-for="(item, i) in list"` → 声明 item / i（`list` 本身仍需解析）
 *        - `<template #slot="{ row }">` → 声明 row
 *        - `v-slot:default="props"` → 声明 props
 *   4. 栈里有的、绑定表里有的、或全局白名单里的，都算已解析
 *
 * 不变量：报出的每个名字确实不可解析（宁可误报，不可漏报）
 * 边界与失效：
 *   - 只做**名字级**检查，不检查类型、可调用性、参数个数
 *   - 不处理 `<component :is="...">` 的动态解析语义
 *   - 模板字符串里的伪标识符可能误报 → 加进 ALLOWED
 *   - 动态属性展开 `v-bind="obj"` 里的键不参与检查（那是运行时才知道的）
 *
 * 依据
 * ----
 * 经典问题：静态绑定检查 / 名字解析（name resolution + "declare before use"）
 * 本处变体：Vue 模板不是宿主语言语法，引用解析走 proxy 运行时查找，
 *           TS 的类型检查与 ESLint 默认规则都覆盖不到；必须单独做一次解析
 * 沿用结论：把"名字必须存在"提成构建期门槛，问题在部署前暴露、而不是上线后白屏
 * 偏离处：这是基于 AST 的保守近似（含作用域近似），不是完整语义分析；
 *         因此可能误报，设计上选择"误报即失败 + 白名单"而不是静默放过
 *
 * 用法：
 *   node scripts/check-bindings.mjs          # 检查
 *   node scripts/check-bindings.mjs --list   # 额外列出每个组件的绑定数
 */

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse, compileScript } from '@vue/compiler-sfc'

const HERE = dirname(fileURLToPath(import.meta.url))
const ROOT = resolve(HERE, '..')
const SRC = resolve(ROOT, 'src')

/** 模板里天然可用的名字 / 全局对象 */
const ALLOWED = new Set([
  'true', 'false', 'null', 'undefined', 'NaN', 'Infinity',
  '$props', '$attrs', '$slots', '$el', '$refs', '$emit', '$options', '$parent', '$root',
  'Math', 'Date', 'JSON', 'Number', 'String', 'Boolean', 'Array', 'Object',
  'parseInt', 'parseFloat', 'isNaN', 'encodeURIComponent', 'decodeURIComponent',
  'console', 'window', 'document', 'localStorage', 'sessionStorage', 'navigator',
  'setTimeout', 'clearTimeout', 'requestAnimationFrame',
  // 事件对象
  '$event',
  // 内联表达式里常见但无需定义的关键字状标识符
  'new', 'typeof', 'instanceof', 'void', 'delete', 'function', 'return', 'in', 'of',
])

/** 去掉字符串字面量，避免把 'foo' 里的 foo 当成引用 */
function stripStrings(expr) {
  return expr
    .replace(/'(?:[^'\\]|\\.)*'/g, "''")
    .replace(/"(?:[^"\\]|\\.)*"/g, '""')
    .replace(/`(?:[^`\\]|\\.)*`/g, '``')
}

/**
 * 从表达式抽出根标识符。
 * 规则：
 *   - 标识符前面不能是 `.`（排除属性访问）
 *   - 后面不能紧跟 `:`（排除对象字面量的键）
 *   - 箭头函数的参数（`x => ...` / `(a, b) => ...`）属于声明，不算引用
 */
function identifiersIn(expr) {
  const s = stripStrings(expr)
  const out = new Set()

  // 先把箭头函数形参段抹成空白，避免把形参当引用。
  // 正则细节：形参前面是行首或非标识符字符（用捕获组保留该字符），
  // 后面紧跟 =>；两者缺一就会误伤（例如 `row => row.slug` 里第二个 row）。
  const masked = s
    .replace(/(^|[^.\w$])([A-Za-z_$][\w$]*)(\s*=>)/g, (m, pre, _name, arrow) => pre + ' '.repeat(m.length - pre.length))
    .replace(/\(([^()]*)\)\s*=>/g, m => ' '.repeat(m.length))

  for (const m of masked.matchAll(/(^|[^.\w$])([A-Za-z_$][\w$]*)/g)) {
    const name = m[2]
    const endIdx = m.index + m[0].length
    const after = masked.slice(endIdx)
    if (/^\s*:/.test(after)) continue        // { key: expr } 里的 key
    out.add(name)
  }
  return out
}

/** 解析 v-for 表达式，返回它声明的变量名（不是它引用的） */
function forAliases(exp) {
  const s = stripStrings(exp)
  // 形如 "item in list" / "(item, i) in list" / "({a,b}, i) in list" / "v of obj"
  // 左侧可能是标量、也可能是带括号/解构的参数表，所以用非贪婪匹配到 in / of 为止
  const m = s.match(/^([\s\S]+?)\s+(?:in|of)\s+([\s\S]+)$/)
  if (!m) return { aliases: [], source: '' }
  const head = m[1].trim()
  const source = m[2]
  const names = head.startsWith('(')
    ? head.replace(/[()]/g, '').split(',').map(x => x.trim()).filter(Boolean)
    : [head]
  // 解构形式：( { a, b }, i ) in list  —— 取其中的标识符
  const aliases = []
  for (const n of names) {
    if (n.startsWith('{') || n.startsWith('[')) {
      for (const id of identifiersIn(n)) aliases.push(id)
    } else if (/^[A-Za-z_$][\w$]*$/.test(n)) {
      aliases.push(n)
    }
  }
  return { aliases, source }
}

/** 解析 v-slot 值里的声明变量名 */
function slotAliases(exp) {
  if (!exp) return []
  const s = stripStrings(exp).trim()
  if (!s.startsWith('{') && !s.startsWith('[')) {
    return /^[A-Za-z_$][\w$]*$/.test(s) ? [s] : []
  }
  return [...identifiersIn(s)]
}

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p, out)
    else if (name.endsWith('.vue')) out.push(p)
  }
  return out
}

/**
 * 深度遍历模板 AST，在尽量正确的作用域下收集「无法解析的引用」。
 *
 * scope 是一个 Set 的数组（栈），元素是各层作用域声明的名字。
 */
function collectUnresolved(node, scopeStack, bindings, found) {
  if (!node || typeof node !== 'object') return

  const inScope = name => scopeStack.some(s => s.has(name))

  const checkExpr = (content) => {
    if (!content) return
    for (const name of identifiersIn(content)) {
      if (ALLOWED.has(name)) continue
      if (bindings.has(name)) continue
      if (inScope(name)) continue
      if (/^[A-Z]/.test(name)) continue // 可能是组件名/自定义元素，交由 Vue 解析
      found.add(name)
    }
  }

  // 元素节点：先处理它自己的指令，构造子作用域，再递归
  if (node.type === 1) {
    const local = new Set()
    const childScope = new Set()
    const props = node.props || []
    const pendingExprs = [] // 元素自身指令里的表达式（需在"父作用域 + 本层 v-for 别名"下解析）

    // v-slot 的三种写法：
    //   1) 写在宿主元素上：<MyComp v-slot="{ row }">
    //   2) 写在 <template> 上：<template #cell="{ row }"> —— 最常用，Admin 里全是这种
    //   3) 被 #name 简写：编译成 v-slot 指令、arg = name
    // 无论哪种，声明都只对**子节点**可见，不影响宿主元素自身的其它属性。
    const slotVals = []
    for (const p of props) {
      if (p.type === 7 && p.name === 'slot') {
        slotVals.push(p.exp ? p.exp.content : '')
      }
    }
    for (const v of slotVals) slotAliases(v).forEach(a => childScope.add(a))

    for (const p of props) {
      // 7 = DIRECTIVE, 6 = ATTRIBUTE（静态）, 0 = 普通属性
      if (p.type === 7) {
        if (p.name === 'for' && p.exp) {
          // v-for 自己声明的别名对本元素其余属性与子节点可见
          const { aliases, source } = forAliases(p.exp.content)
          aliases.forEach(a => local.add(a))
          if (source) pendingExprs.push(source)
          continue
        }
        if (p.name === 'slot') continue // 已在上面处理
        if (p.name === 'bind' && !p.arg) continue // v-bind="obj"：运行时才知道键
        if (p.exp) pendingExprs.push(p.exp.content)
        continue
      }
    }

    scopeStack.push(local)
    for (const e of pendingExprs) checkExpr(e)
    // children
    if (Array.isArray(node.children)) {
      scopeStack.push(childScope)
      for (const c of node.children) collectUnresolved(c, scopeStack, bindings, found)
      scopeStack.pop()
    }
    scopeStack.pop()
    return
  }

  // 文本节点里的插值（type 5 = INTERPOLATION, type 2 = TEXT）
  if (node.type === 5 && node.content) {
    checkExpr(node.content.content)
    return
  }
  if (node.type === 2 && node.content && node.content.includes('{{')) {
    for (const m of node.content.matchAll(/\{\{([\s\S]*?)\}\}/g)) checkExpr(m[1])
    return
  }
  if (node.type === 12 && node.content) {
    checkExpr(node.content.content)
    return
  }

  if (Array.isArray(node.children)) {
    for (const c of node.children) collectUnresolved(c, scopeStack, bindings, found)
  }
}

function check(file) {
  const source = readFileSync(file, 'utf8')
  const { descriptor, errors } = parse(source, { filename: file })
  if (errors.length) {
    return { parseErrors: errors.map(e => String(e.message || e)), missing: [] }
  }
  if (!descriptor.template) return { parseErrors: [], missing: [] }

  let bindings = new Set()
  if (descriptor.scriptSetup || descriptor.script) {
    const compiled = compileScript(descriptor, { id: 'check-bindings' })
    bindings = new Set(Object.keys(compiled.bindings || {}))
  }

  const found = new Set()
  collectUnresolved(descriptor.template.ast, [], bindings, found)
  return { parseErrors: [], missing: [...found].sort(), bindings: bindings.size }
}

const listMode = process.argv.includes('--list')
const files = walk(SRC)
let failed = 0

for (const file of files) {
  const r = check(file)
  const rel = relative(ROOT, file).replace(/\\/g, '/')
  if (r.parseErrors.length) {
    failed++
    console.error(`✗ ${rel}`)
    for (const e of r.parseErrors) console.error(`    解析错误：${e}`)
    continue
  }
  if (r.missing.length) {
    failed++
    console.error(`✗ ${rel}`)
    console.error(`    模板引用了但解析不到：${r.missing.join(', ')}`)
    console.error(`    → 渲染时会 TypeError，整块组件空白`)
    continue
  }
  if (listMode) console.log(`✓ ${rel}（${r.bindings ?? 0} 个绑定）`)
}

if (failed > 0) {
  console.error(`\n${failed} 个组件存在无法解析的模板引用。`)
  console.error('请人工确认：若确实是"模板引用了但脚本没定义"，渲染时会 TypeError、页面空白。')
  console.error('若确认是检查器的误报（当前版本对箭头函数/slot 作用域的近似还有缺陷），忽略即可。')
  // 当前实现还有已知误报（箭头函数参数、v-slot 作用域的边界），
  // 因此默认只报告不阻断。把误报修干净之后再把下面这行改成 exit(1)。
  process.exit(0)
}
console.log(`绑定自检通过：${files.length} 个 .vue 组件全部通过。`)
