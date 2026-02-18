# 科研信息提取Agent开发进程

## 项目概述
自动化科研信息提取系统，支持关键词订阅、多模态内容解析、中英文对照、PPT生成等功能。

---

## 项目架构设计

### 1. 核心模块划分

```
bsxbot/
├── src/
│   ├── main.rs                 # 程序入口、调度器
│   ├── config/
│   │   ├── mod.rs             # 配置管理
│   │   └── keywords.rs        # 关键词订阅管理
│   ├── crawler/
│   │   ├── mod.rs             # 爬虫模块入口
│   │   ├── arxiv.rs           # arXiv论文爬取
│   │   ├── pubmed.rs          # PubMed医学文献
│   │   ├── scholar.rs         # Google Scholar
│   │   └── semantic.rs        # Semantic Scholar API
│   ├── parser/
│   │   ├── mod.rs             # 解析器入口
│   │   ├── pdf_parser.rs      # PDF文本提取
│   │   ├── formula_extractor.rs  # LaTeX公式提取
│   │   ├── image_analyzer.rs  # 图像信息提取
│   │   └── table_parser.rs    # 表格结构解析
│   ├── translator/
│   │   ├── mod.rs             # 翻译模块
│   │   └── bilingual.rs       # 中英文对照生成
│   ├── generator/
│   │   ├── mod.rs             # 生成器入口
│   │   ├── ppt_generator.rs   # PPT生成
│   │   └── report_builder.rs  # 报告构建
│   ├── storage/
│   │   ├── mod.rs             # 存储层
│   │   ├── database.rs        # 数据库操作
│   │   └── cache.rs           # 缓存管理
│   └── utils/
│       ├── mod.rs
│       ├── scheduler.rs       # 定时任务
│       └── logger.rs          # 日志系统
├── data/
│   ├── papers/                # 下载的论文
│   ├── images/                # 提取的图像
│   └── reports/               # 生成的报告
├── config/
│   ├── keywords.toml          # 关键词配置
│   └── settings.toml          # 系统配置
└── Cargo.toml
```

### 2. 技术栈选型

#### 核心依赖
- **HTTP客户端**: `reqwest` (异步HTTP请求)
- **HTML解析**: `scraper` (网页内容提取)
- **PDF处理**: `pdf-extract` / `lopdf` (PDF解析)
- **图像处理**: `image` (图像读取和分析)
- **异步运行时**: `tokio` (异步任务调度)
- **数据库**: `sqlx` + `sqlite` (轻量级存储)
- **序列化**: `serde` + `serde_json` (数据序列化)
- **定时任务**: `tokio-cron-scheduler` (定时爬取)

#### AI/ML相关
- **翻译API**: OpenAI API / Claude API / DeepL API
- **公式识别**: `latex` crate + OCR (Tesseract)
- **图像理解**: Vision API (GPT-4V / Claude Vision)
- **PPT生成**: `rust-pptx` 或调用Python `python-pptx`

---

## 关键技术点

### 1. 数据源接入
- **arXiv API**: 免费、无需认证、支持RSS订阅
- **PubMed E-utilities**: 需要API key，限流控制
- **Semantic Scholar API**: 免费tier，需注册
- **注意**: 遵守robots.txt，实现请求限流和重试机制

### 2. PDF多模态解析
```rust
// 解析流程
PDF文件
  ├─> 文本提取 (pdf-extract)
  ├─> 公式提取 (正则匹配LaTeX语法)
  ├─> 图像提取 (lopdf提取嵌入图像)
  └─> 表格识别 (基于布局分析)
```

### 3. 中英文对照实现
- 段落级别对齐（保持原文结构）
- 专业术语词典（领域特定翻译）
- 公式保持原样（不翻译LaTeX）
- 引用格式保留

### 4. PPT生成策略
```
报告结构:
├─ 封面页 (标题、日期、关键词)
├─ 摘要页 (核心发现总结)
├─ 论文列表 (每篇论文一页)
│   ├─ 标题 (中英对照)
│   ├─ 作者和机构
│   ├─ 核心贡献 (3-5点)
│   ├─ 关键公式 (1-2个)
│   └─ 重要图表 (1-2张)
└─ 趋势分析页 (可选)
```

---

## 开发阶段规划

### Phase 1: 基础框架 (Week 1-2)
- [x] 项目初始化
- [ ] 配置系统实现
- [ ] 日志和错误处理
- [ ] 数据库schema设计
- [ ] 基础爬虫框架

### Phase 2: 数据采集 (Week 3-4)
- [ ] arXiv爬虫实现
- [ ] PubMed爬虫实现
- [ ] 关键词匹配算法
- [ ] 去重和增量更新
- [ ] 定时任务调度

### Phase 3: 内容解析 (Week 5-6)
- [ ] PDF文本提取
- [ ] LaTeX公式识别
- [ ] 图像提取和分析
- [ ] 表格结构解析
- [ ] 元数据提取

### Phase 4: 智能处理 (Week 7-8)
- [ ] 翻译API集成
- [ ] 中英对照生成
- [ ] 内容摘要生成
- [ ] 关键信息提取

### Phase 5: 报告生成 (Week 9-10)
- [ ] PPT模板设计
- [ ] 自动排版算法
- [ ] 图表嵌入
- [ ] 多格式导出 (PPTX/PDF/Markdown)

### Phase 6: 优化和部署 (Week 11-12)
- [ ] 性能优化
- [ ] 错误恢复机制
- [ ] 用户界面 (CLI/Web)
- [ ] 文档和测试

---

## 关键注意事项

### 1. 法律和伦理
- ⚠️ 遵守各平台的ToS和API使用条款
- ⚠️ 尊重版权，不公开分发论文全文
- ⚠️ 实现合理的请求频率限制
- ⚠️ 标注数据来源和引用

### 2. 技术挑战
- **PDF解析准确性**: 不同期刊格式差异大，需要多种解析策略
- **公式识别**: LaTeX语法复杂，需要robust的正则表达式
- **图像理解**: 需要Vision API，成本较高
- **翻译质量**: 专业术语翻译需要领域知识
- **并发控制**: 避免被封IP，实现智能限流

### 3. 性能优化
- 使用异步IO处理并发请求
- 实现本地缓存减少重复请求
- 增量更新而非全量爬取
- 图像压缩和懒加载
- 数据库索引优化

### 4. 可扩展性
- 插件化的数据源接口
- 可配置的解析规则
- 模板化的报告生成
- 支持自定义关键词和过滤规则

---

## 配置文件示例

### keywords.toml
```toml
[[subscriptions]]
name = "机器学习"
keywords = ["machine learning", "deep learning", "neural network"]
sources = ["arxiv", "semantic_scholar"]
categories = ["cs.LG", "cs.AI"]
enabled = true

[[subscriptions]]
name = "计算机视觉"
keywords = ["computer vision", "image recognition", "object detection"]
sources = ["arxiv", "pubmed"]
categories = ["cs.CV"]
enabled = true
```

### settings.toml
```toml
[crawler]
max_papers_per_day = 50
request_delay_ms = 1000
user_agent = "ResearchBot/1.0"

[translator]
api_provider = "openai"  # openai, claude, deepl
api_key = "your-api-key"
target_language = "zh-CN"

[generator]
ppt_template = "academic"
max_papers_per_report = 20
include_images = true
include_formulas = true

[storage]
database_path = "./data/papers.db"
cache_ttl_days = 30
```

---

## API集成建议

### 1. arXiv API
```rust
// 示例查询
// http://export.arxiv.org/api/query?search_query=cat:cs.AI+AND+submittedDate:[20260217+TO+20260218]&max_results=100
```

### 2. Semantic Scholar API
```rust
// 需要注册获取API key
// https://api.semanticscholar.org/graph/v1/paper/search?query=machine+learning&fields=title,authors,abstract
```

### 3. 翻译API选择
- **OpenAI GPT-4**: 质量最高，成本较高，支持专业术语
- **Claude API**: 质量高，上下文窗口大，适合长文本
- **DeepL API**: 性价比高，翻译自然，但专业术语可能不准确

---

## 下一步行动

1. **立即开始**: 实现配置系统和基础框架
2. **优先级**: 先完成arXiv爬虫（最简单、最稳定）
3. **测试驱动**: 每个模块编写单元测试
4. **迭代开发**: 先实现MVP，再逐步增加功能

---

## 开发日志

### 2026-02-18
- ✅ 项目初始化
- ✅ 架构设计完成
- ✅ 技术栈选型
- ✅ 创建开发进程文档
- ✅ 配置系统实现（AppConfig + KeywordConfig）
- ✅ 日志系统实现（tracing）
- ✅ 错误处理框架（thiserror + anyhow）
- ✅ 数据库schema设计（SQLite + sqlx）
- ✅ 基础爬虫框架（arXiv爬虫）
- ✅ 定时任务调度器（tokio-cron-scheduler）
- ✅ 缓存系统实现
- ✅ CLI命令行工具（init/crawl/schedule/report）
- ✅ 项目编译通过
- 📝 下一步: 完善arXiv XML解析，实现PDF下载功能

### 待更新...

---

## 参考资源

- [arXiv API文档](https://arxiv.org/help/api)
- [Semantic Scholar API](https://www.semanticscholar.org/product/api)
- [PubMed E-utilities](https://www.ncbi.nlm.nih.gov/books/NBK25501/)
- [python-pptx文档](https://python-pptx.readthedocs.io/)
