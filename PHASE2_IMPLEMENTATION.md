# Phase 2 实现说明

## 已完成的功能

### 1. arXiv XML解析
- ✅ 实现了简单的XML解析（基于字符串处理）
- ✅ 提取论文元数据：标题、作者、摘要、发布日期、分类
- ✅ 自动生成PDF下载链接
- ✅ 按提交日期降序排序

### 2. PDF下载
- ✅ 异步下载PDF文件
- ✅ 保存到 `data/papers/` 目录
- ✅ 文件名格式：`arxiv_id.pdf`
- ✅ 错误处理和日志记录

### 3. PDF文本提取
- ✅ 使用 `pdf-extract` 库提取文本
- ✅ 实现 `extract_first_lines()` 方法提取前N行
- ✅ 实现 `extract_full_text()` 方法提取完整文本
- ✅ 过滤空行

### 4. 数据库操作
- ✅ `save_paper()` - 保存论文到数据库
- ✅ `paper_exists()` - 检查论文是否已存在
- ✅ `update_pdf_path()` - 更新PDF路径
- ✅ 使用 UPSERT 避免重复

### 5. 配置更新
- ✅ 将API提供商改为 MiniMax
- ✅ 添加 `api_url` 和 `model` 配置项
- ✅ 默认模型：`abab6.5-chat`

## 工作流程

```
用户执行: bsxbot crawl
    ↓
1. 加载配置（keywords.toml）
    ↓
2. 调用 arXiv API 搜索论文
    ↓
3. 解析 XML 响应
    ↓
4. 遍历前3篇论文：
    ├─ 检查数据库是否已存在
    ├─ 保存论文元数据到数据库
    ├─ 下载 PDF 文件
    ├─ 提取 PDF 前两行文本
    └─ 打印到控制台
    ↓
5. 完成
```

## 测试方法

```bash
# 1. 重新初始化（如果需要）
./target/release/bsxbot.exe init

# 2. 编辑配置文件
# 修改 config/keywords.toml 中的关键词

# 3. 执行爬取
./target/release/bsxbot.exe crawl

# 4. 查看结果
# - 数据库：data/papers.db
# - PDF文件：data/papers/*.pdf
# - 日志输出：控制台
```

## 示例输出

```
INFO bsxbot: 开始爬取任务...
INFO bsxbot: 处理订阅: 机器学习
INFO bsxbot: 关键词: ["machine learning", "deep learning", "neural network"]
INFO bsxbot::crawler::arxiv: 正在搜索 arXiv: http://...
INFO bsxbot::crawler::arxiv: 找到 50 篇论文
INFO bsxbot: ---
INFO bsxbot: 标题: Attention Is All You Need
INFO bsxbot: 作者: Ashish Vaswani, Noam Shazeer, ...
INFO bsxbot: 发布日期: 2017-06-12T17:57:34Z
INFO bsxbot: PDF: http://arxiv.org/pdf/1706.03762.pdf
INFO bsxbot: 论文已保存到数据库，ID: 1
INFO bsxbot::crawler::arxiv: 下载PDF: http://... -> data/papers/1706.03762.pdf
INFO bsxbot::crawler::arxiv: PDF下载完成: data/papers/1706.03762.pdf
INFO bsxbot::parser::pdf_parser: 解析PDF: data/papers/1706.03762.pdf
INFO bsxbot::parser::pdf_parser: 成功提取 2 行文本
INFO bsxbot: PDF前两行内容:
INFO bsxbot:   第1行: Attention Is All You Need
INFO bsxbot:   第2行: Ashish Vaswani, Noam Shazeer, Niki Parmar, Jakob Uszkoreit, ...
```

## 依赖更新

```toml
quick-xml = "0.31"  # XML解析
pdf-extract = "0.7"  # PDF文本提取
lopdf = "0.32"       # PDF底层操作（备用）
```

## 已知限制

1. XML解析使用简单的字符串处理，不够健壮
2. 只处理前3篇论文（测试用）
3. PDF解析可能对扫描版PDF效果不佳
4. 没有实现并发下载
5. 没有断点续传功能

## 下一步优化

1. 使用 `quick-xml` 进行完整的XML解析
2. 实现并发下载（tokio::spawn）
3. 添加下载进度条
4. 实现PDF解析质量检测
5. 添加更多错误恢复机制
6. 实现翻译功能（MiniMax API）

## MiniMax API 配置

```toml
[translator]
api_provider = "minimax"
api_key = "your-minimax-api-key"
api_url = "https://api.minimax.chat/v1/text/chatcompletion_v2"
model = "abab6.5-chat"
target_language = "zh-CN"
```

API文档：https://www.minimaxi.com/document/guides/chat-model/V2
