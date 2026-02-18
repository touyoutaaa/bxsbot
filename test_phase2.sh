#!/bin/bash

echo "=== Phase 2 测试脚本 ==="
echo ""

# 1. 编译项目
echo "1. 编译项目..."
cd "D:\project\bsxbot"
cargo build --release 2>&1 | tail -5

if [ $? -ne 0 ]; then
    echo "❌ 编译失败"
    exit 1
fi

echo "✅ 编译成功"
echo ""

# 2. 测试爬取功能
echo "2. 测试爬取功能（限制3篇论文）..."
./target/release/bsxbot.exe crawl --subscription "机器学习"

echo ""
echo "=== 测试完成 ==="
