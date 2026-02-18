# bsxbot - 科研信息自动提取与分析系统

自动化科研信息提取系统，支持关键词订阅、多模态内容解析、中英文对照、PPT生成等功能。

## 快速开始

### 1. 初始化系统

```bash
cargo run -- init
```

这将创建必要的目录结构和配置文件：
- `config/settings.toml` - 系统配置
- `config/keywords.toml` - 关键词订阅配置
- `data/papers.db` - SQLite数据库

### 2. 配置API密钥

编辑 `config/settings.toml`，填入你的API密钥：

```toml
[translator]
api_provider = "openai"
api_key = "sk-your-api-key-here"
```

### 3. 配置研究方向

编辑 `config/keywords.toml`，添加你关注的研究方向：

```toml
[[subscriptions]]
name = "机器学习"
keywords = ["machine learning", "deep learning"]
sources = ["arxiv"]
categories = ["cs.LG", "cs.AI"]
enabled = true
```

### 4. 运行爬虫

```bash
# 爬取所有启用的订阅
cargo run -- crawl

# 爬取特定订阅
cargo run -- crawl --subscription "机器学习"
```

### 5. 启动定时任务

```bash
cargo run -- schedule
```

每天早上8点自动执行爬取任务。

### 6. 生成报告

```bash
# 生成今天的报告
cargo run -- report

# 生成指定日期的报告
cargo run -- report --date 2026-02-18
```

## 项目结构

```
bsxbot/
├── src/
│   ├── main.rs           # 程序入口
│   ├── config/           # 配置管理
│   ├── crawler/          # 爬虫模块
│   ├── parser/           # 解析模块
│   ├── translator/       # 翻译模块
│   ├── generator/        # 报告生成
│   ├── storage/          # 数据存储
│   └── utils/            # 工具函数
├── config/               # 配置文件
├── data/                 # 数据目录
│   ├── papers/          # 下载的论文
│   ├── images/          # 提取的图像
│   └── reports/         # 生成的报告
└── CLAUDE.md            # 开发文档
```

## 功能特性

- ✅ 配置系统
- ✅ 日志系统
- ✅ 数据库设计
- ✅ 基础爬虫框架（arXiv）
- ✅ 定时任务调度
- ⏳ PDF解析
- ⏳ 公式提取
- ⏳ 图像分析
- ⏳ 中英文翻译
- ⏳ PPT生成

## 开发

```bash
# 编译
cargo build

# 运行测试
cargo test

# 检查代码
cargo clippy
```

## 许可证

MIT
