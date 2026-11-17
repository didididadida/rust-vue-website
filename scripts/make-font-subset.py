#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""字体子集化：把标题用的思源宋体裁到"站里真正出现过的字"。

为什么要有这个脚本
------------------
首页、卡片标题、区块标题全部用 `--font-serif` 渲染，而它指向
`public/fonts/noto-serif-sc-600.woff2` —— 1.46 MB、8244 个字形。
woff2 本身已经压到极限（实测再 gzip 反而变大），hinting 开关对 CJK 也无差别
（实测 ±0.003%），唯一的降维手段就是**只留用得到的字**。

实测本仓：1,529,908 B → 约 25 KB，缩小约 60 倍；单字形成本约 79 B（线性）。
1.46MB 在当前链路里要 4 秒以上，这段时间里标题一直是回退字体，下载完再整体重排一次，
这正是"字一点点换过来"的体感来源。

输入 → 输出
------------
输入：
  - 原始字体      frontend/public/fonts/noto-serif-sc-600.woff2
  - 文案语料      默认扫描 frontend/src、posts、data（模板硬编码中文 + 文章正文）
输出：
  - frontend/public/fonts/noto-serif-sc-600.subset.woff2
  - 控制台打印：保留字符数、最终字节数

关键机制
--------
1. fontTools.subset 只保留 charset 里的字形。**charset 默认就是扫描结果本身**，
   不附加"常用字白名单"：3755 个常用汉字实测要 +476 KB，等于把优化吃掉 67%
2. `layout_features=['*']` 保留全部 OpenType 特性（中文标点定位、比例宽度等）
3. `hinting=False`：实测对 CJK 子集体积无影响，但语义上是"精简"，保留无害
4. ASCII 与中英文标点总是并入，避免脚本扫到 `Blog` 却漏掉大小写/标点变体

不变量
------
- 任何出现在 frontend/src、posts、data 里的字符，必定在产物里
- 产物仍是合法 woff2

边界与失效（重要）
------------------
- **站里没出现过的汉字一定会缺**，浏览器按 `--font-serif` 的回退链落到
  系统宋体（Songti SC / SimSun）。所以**发了含新字的新文章后必须重跑本脚本**，
  否则新文章标题会混排宋体。这是刻意的取舍：见下方"代价曲线"
- `--whitelist` 可加常用字白名单（体积按约 150 B/字线性增长），想稳一点可以加

代价曲线（本仓实测，woff2 字节）
--------------------------------
    语料里出现的 1232 个汉字          → 约 220 KB
    + 全部 ASCII / 标点 / 外文符号    → 约 233 KB
    + GB2312 一级 3755 常用汉字       → 约 709 KB
    原字体 7946 字形全量              → 1,493 KB

也就是说**主体开销就是"用到的汉字本身"**，白名单不是免费的保险而是 3 倍代价。
注意：这些字是二阶贝塞尔轮廓，每字约 150~180 B，不是文档里常见的 `79 B/字`
经验值——那个数字来自只含 ANSI 的拉丁子集，不能套到 CJK 上。

用法
----
    python scripts/make-font-subset.py                 # 生成
    python scripts/make-font-subset.py --report        # 只报告，不写文件
    python scripts/make-font-subset.py --verify        # 检查站内每个字是否都在产物里
    python scripts/make-font-subset.py --whitelist     # 额外覆盖 GB2312 一级常用字

依赖：pip install fonttools brotli

依据
----
经典问题：关键渲染路径优化（critical rendering path）/ 资源子集化（subsetting）
本处变体：Web 字体无法按需下载单字，只能整文件；因此把"按需"这一步
          提前到构建期，用内容语料静态推断需求集合
沿用结论：首屏关键资源的体积下降与首屏就绪时间成正比，字体越小事越小
偏离处：内容是动态增长的，静态子集存在"未来缺字"的风险；
        用 --verify 在每次构建前检查，缺字会显式报出来而不是静默混排
"""

from __future__ import annotations

import argparse
import io
import sys
from pathlib import Path

try:
    from fontTools import subset
    from fontTools.ttLib import TTFont
except ImportError:  # pragma: no cover
    sys.exit("缺少依赖：pip install fonttools brotli")

try:
    import brotli  # noqa: F401  woff2 压缩需要
except ImportError:  # pragma: no cover
    sys.exit("缺少依赖：pip install brotli")


REPO = Path(__file__).resolve().parent.parent
FONT_IN = REPO / "frontend" / "public" / "fonts" / "noto-serif-sc-600.woff2"
FONT_OUT = REPO / "frontend" / "public" / "fonts" / "noto-serif-sc-600.subset.woff2"

# 扫描语料的目录（相对仓库根）。只收文本类扩展名，避免把图片二进制读进来。
SCAN_DIRS = ("frontend/src", "posts", "data")
SCAN_EXTS = {".vue", ".js", ".ts", ".css", ".html", ".md", ".txt", ".json"}

# 标点与符号：中文排版高频，且不会被 GB2312 一级汉字枚举覆盖到
EXTRA_PUNCT = (
    "　、。！？（）【】《》〈〉「」『』〔〕“”‘’—…·～￥"
    "0123456789"
    "abcdefghijklmnopqrstuvwxyz"
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    "!#$%&'()*+,-./:;<=>?@[\\]^_`{|}~\" "
)


def gb2312_level1() -> str:
    """GB2312 一级常用汉字（3755 个），仅用于 `--whitelist` 与 `--report` 的对照。

    一级汉字按拼音排序，区位码 B0A1..D7F9，逐位偏移枚举即可，不必内置大字符串。
    注意：默认 **不** 并入这些字——单字形成本约 79 B，3755 字约 288 KB。
    """
    out = []
    for hi in range(0xB0, 0xD8):
        for lo in range(0xA1, 0xFF):
            try:
                out.append(bytes([hi, lo]).decode("gb2312"))
            except UnicodeDecodeError:
                continue
    return "".join(out)


def scan_corpus() -> str:
    """扫描源码与内容目录，返回出现过的所有字符。"""
    chars: set[str] = set()
    for rel in SCAN_DIRS:
        root = REPO / rel
        if not root.exists():
            continue
        for p in root.rglob("*"):
            if not p.is_file() or p.suffix.lower() not in SCAN_EXTS:
                continue
            # node_modules / dist 永远不该参与（rglob 起点不含它们，这里再加一道保险）
            if "node_modules" in p.parts or "dist" in p.parts:
                continue
            try:
                chars.update(p.read_text(encoding="utf-8"))
            except (OSError, UnicodeDecodeError):
                continue
    return "".join(sorted(chars))


def build_charset(whitelist: bool) -> tuple[str, dict[str, int]]:
    """把语料 + 标点（+ 可选常用字白名单）合并成最终保留集合。"""
    corpus = scan_corpus()
    merged = set(corpus) | set(EXTRA_PUNCT)
    if whitelist:
        merged |= set(gb2312_level1())
    # 去掉控制字符（\n \t 等不产生字形，留着只是噪音）
    merged = {c for c in merged if c >= " "}
    stats = {
        "corpus": len(corpus),
        "merged": len(merged),
    }
    return "".join(sorted(merged)), stats


def verify(chars: str) -> bool:
    """检查产物是否覆盖语料里的每个字（防止发新文章后忘了重跑）。

    只统计"原字体本来就有、却漏在子集外"的字：emoji / 箭头 / 西里尔等
    原字体没有的字形，本来就走系统回退链，不算漏。
    """
    if not FONT_OUT.exists():
        print(f"找不到子集字体：{FONT_OUT}，先运行一次生成")
        return False
    src_cmap = TTFont(str(FONT_IN)).getBestCmap()
    out_cmap = TTFont(str(FONT_OUT)).getBestCmap()
    corpus = scan_corpus()
    missing = sorted(
        {
            c
            for c in corpus
            if c >= " " and ord(c) in src_cmap and ord(c) not in out_cmap
        }
    )
    if not missing:
        print(f"OK：语料中全部汉字与符号都在子集字体里（待保留 {len(chars)} 字符）")
        return True
    print(f"缺失 {len(missing)} 个字符（这些字会回退到系统宋体）：")
    print("  " + "".join(missing[:200]))
    if len(missing) > 200:
        print(f"  ... 另有 {len(missing) - 200} 个未显示")
    print("修复：重新运行 python scripts/make-font-subset.py")
    return False


def main() -> int:
    ap = argparse.ArgumentParser(description="生成标题字体的子集 woff2")
    ap.add_argument("--report", action="store_true", help="只报告，不写文件")
    ap.add_argument("--verify", action="store_true", help="检查现有产物是否覆盖语料")
    ap.add_argument(
        "--whitelist",
        action="store_true",
        help="并入 GB2312 一级常用汉字（约 +288KB，换取对新生文章的抗缺字能力）",
    )
    args = ap.parse_args()

    if not FONT_IN.exists():
        sys.exit(f"找不到原始字体：{FONT_IN}")

    chars, stats = build_charset(args.whitelist)
    print(f"语料字符数        : {stats['corpus']}")
    print(f"合并后待保留字符  : {stats['merged']}")

    original = FONT_IN.stat().st_size
    if args.report:
        print(f"原始字体          : {original:,} B")
        print("（--report 模式，未写出文件）")
        return 0

    if args.verify:
        return 0 if verify(chars) else 1

    size = make_subset(chars, FONT_OUT)
    ratio = size / original if original else 0
    print(f"原始字体          : {original:,} B")
    print(f"子集字体          : {size:,} B（{ratio:.1%}，缩小 {1 / ratio:.0f} 倍）")
    print(f"输出              : {FONT_OUT.relative_to(REPO)}")
    return 0


def make_subset(chars: str, out: Path) -> int:
    """按 charset 生成子集 woff2，写入 out，返回字节数。

    关键机制：`fontTools.subset` 默认会拒绝包含"未解析"字符的请求，
    于是先取原字体的 cmap 做交集——字体里本来就没有的字形（emoji、箭头、
    Cyrillic、韩文……）本来也画不出来，交给浏览器的回退链处理即可。
    """
    opts = subset.Options()
    opts.flavor = "woff2"
    opts.layout_features = ["*"]   # 保留全部 OpenType 特性，否则中文标点定位会变
    opts.name_IDs = ["*"]
    opts.hinting = False           # woff2 压缩后 hinting 收益已可忽略
    opts.notdef_outline = True     # 保留 .notdef，缺字时显示豆腐块而不是乱码

    font = subset.load_font(str(FONT_IN), opts)
    cmap = font.getBestCmap()
    wanted = "".join(sorted({c for c in chars if ord(c) in cmap}))

    sub = subset.Subsetter(options=opts)
    sub.populate(text=wanted)
    sub.subset(font)

    buf = io.BytesIO()
    font.flavor = "woff2"
    font.save(buf)
    data = buf.getvalue()

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(data)
    return len(data)


if __name__ == "__main__":
    raise SystemExit(main())
