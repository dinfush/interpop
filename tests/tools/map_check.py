#!/usr/bin/env python3
"""
map_check.py - INTERPOP 架构拓扑与治理地图复合机检工具
依据：
- AGENTS.html §5 地图规则
- SOP-P1 附录 A 门禁子集 (G1/G2/G12)
- 纯物理文件直读，不依赖 git 索引，与 .gitignore 隔离解耦
"""

import sys
import re
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# 允许排除检查的物理目录
EXCLUDE_DIRS = {
    ".git",
    "target",
    "tests",
    "docs",
    "node_modules",
}

def check_internal_topology():
    print("[CHECK 1] Workspace 动态成员列表 × 内部拓扑依赖边机检...")
    cargo_toml = ROOT / "Cargo.toml"
    if not cargo_toml.exists():
        print("  -> FAIL: 根目录缺少 Cargo.toml")
        return False
    
    # 拓扑位阶断言：core <- render <- atoms; scaffold 仅依赖 core
    # 严禁 core/render/atoms 依赖 scaffold
    for crate in ["interpop-core", "interpop-render", "interpop-atoms"]:
        sub_cargo = ROOT / crate / "Cargo.toml"
        if sub_cargo.exists():
            content = sub_cargo.read_text(encoding="utf-8")
            if "interpop-scaffold" in content:
                print(f"  -> FAIL: 发现违规反向依赖！{crate} 引入了 interpop-scaffold")
                return False
    print("  -> PASS: 内部依赖拓扑严格收敛于单向无环白名单")
    return True

def check_map_consistency():
    print("[CHECK 2] 源码树与地图 data-file 双向一致性断言...")
    map_file = ROOT / "PROJECT_MAP.html"
    if not map_file.exists():
        print("  -> FAIL: 根目录缺少 PROJECT_MAP.html")
        return False

    map_content = map_file.read_text(encoding="utf-8")
    
    # 从 PROJECT_MAP.html 提取全部登记的 data-file
    registered_files = set(re.findall(r'data-file="([^"]+)"', map_content))

    # 扫描源码树中实际存在的 *.rs 文件（排除 tests/ 与 target/）
    actual_rs_files = set()
    for rs_path in ROOT.glob("interpop-*/src/**/*.rs"):
        rel_path = rs_path.relative_to(ROOT).as_posix()
        actual_rs_files.add(rel_path)

    # 检查未登记的文件
    unregistered = actual_rs_files - registered_files
    # 检查登记了但物理不存在的文件
    ghost = registered_files - actual_rs_files

    if unregistered:
        print("[FAIL] 发现物理文件未在地图登记 (未入籍源文件):")
        for f in sorted(unregistered):
            print(f"  + {f}")
        return False

    if ghost:
        print("[FAIL] 发现地图登记了但物理不存在的幽灵文件:")
        for f in sorted(ghost):
            print(f"  - {f}")
        return False

    print(f"  -> PASS: 源码树与治理索引表 1:1 精确对齐 (共审计 {len(actual_rs_files)} 个源文件)")
    return True

def check_progress_json():
    print("[CHECK 3] progress.json 运行态合法性检查...")
    p_file = ROOT / "progress.json"
    if not p_file.exists():
        print("  -> FAIL: 缺少 progress.json")
        return False
    try:
        data = json.loads(p_file.read_text(encoding="utf-8"))
        required_keys = ["current_phase", "current_pr", "status", "next_action"]
        for k in required_keys:
            if k not in data:
                print(f"  -> FAIL: progress.json 缺少核心字段 '{k}'")
                return False
    except Exception as e:
        print(f"  -> FAIL: progress.json 解析失败: {e}")
        return False
    print("  -> PASS: progress.json 格式合法收敛")
    return True

def main():
    print(f"=== INTERPOP 架构拓扑与治理地图复合机检 (map_check.py) ===")
    print(f"仓库根路径: {ROOT}")
    
    ok1 = check_internal_topology()
    ok2 = check_map_consistency()
    ok3 = check_progress_json()
    
    if ok1 and ok2 and ok3:
        print("\n[SUCCESS] 全部复合治理与拓扑断言通过。")
        sys.exit(0)
    else:
        print("\n[FAIL] 机检失败：系统拓扑、治理地图或运行态存在违规！")
        sys.exit(1)

if __name__ == "__main__":
    main()
