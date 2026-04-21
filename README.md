# Amo — AI 情感关系分析平台

Amo（阿默）是一个基于大语言模型的情感关系分析后端服务，帮助用户理解亲密关系中的沟通问题。

## 项目介绍

### 解决什么问题

亲密关系中的沟通困惑是普遍存在的：冷战背后的真实需求是什么？对方的回避是性格还是操控？自己的表达方式哪里出了问题？

Amo 通过 AI 分析聊天记录和冲突描述，帮用户看清关系中的沟通模式、情绪变化和潜在风险，给出具体可操作的建议。不替用户做决定，而是帮用户想清楚。

### 核心功能

- **聊天记录分析**（20 credits）— 上传对话记录，生成包含情绪轨迹、沟通模式、风险标注、核心诉求、改善建议的完整报告
- **冲突分析**（8 credits）— 描述一次争吵或冲突，AI 梳理来龙去脉、分析双方诉求、给出情景分支决策
- **情绪疏导**（2 credits/轮）— 多轮对话，AI 扮演懂心理学的知心朋友，帮用户区分事实与解读、识别认知扭曲
- **追问机制**（3-5 credits）— 对分析报告或冲突分析结果进行深入追问

### 额度系统

使用卡密（格式 `AMO-XXXX-XXXX-XXXX`）进行鉴权和额度管理，支持批量生成、过期时间设置。管理员可通过后台接口管理卡密。

---

## 技术实现

### 技术栈

| 层级 | 技术 |
|------|------|
| 语言 | Rust 2021 Edition |
| Web 框架 | Axum 0.7 |
| 数据库 | PostgreSQL + SQLx 0.8（异步、编译期检查） |
| 异步运行时 | Tokio |
| LLM 集成 | Anthropic Claude / OpenAI（可切换） |
| 流式响应 | SSE（Server-Sent Events）+ async-stream |
| 并发存储 | DashMap（无锁并发 HashMap） |

### 架构设计

```
src/
├── api/              # HTTP 接口层（Axum handlers）
│   ├── analysis.rs   # 聊天记录分析（异步任务 + 轮询）
│   ├── conflict.rs   # 冲突分析 + 追问（SSE 流式）
│   ├── emotional.rs  # 情绪疏导多轮对话（SSE 流式）
│   ├── cards.rs      # 卡密验证与余额查询
│   └── admin.rs      # 管理员卡密管理
├── analysis/         # 分析业务逻辑
│   ├── pipeline.rs   # 5 步分析流水线
│   └── types.rs      # 数据结构定义
├── llm/              # LLM 抽象层
│   ├── mod.rs        # LlmClient trait 定义
│   ├── anthropic.rs  # Anthropic Claude 实现
│   └── openai.rs     # OpenAI 实现
├── middleware/        # 鉴权中间件
├── credits/          # 额度扣减逻辑
├── prompts.rs        # 所有 LLM 系统提示词
├── config.rs         # 环境变量配置
├── state.rs          # 应用状态（DB + LLM + TaskStore）
└── main.rs           # 入口
```

### LLM 双 Provider 抽象

通过 `LlmClient` trait 统一接口，支持 Anthropic 和 OpenAI 两种后端：

- **ModelTier::Smart** — 用于分析任务（claude-sonnet-4-6 / gpt-4o）
- **ModelTier::Fast** — 用于轻量对话（claude-haiku-4-5 / gpt-4o-mini）

两种模式均支持非流式（`complete`）和流式（`stream`）调用。通过环境变量 `LLM_PROVIDER` 切换，支持第三方代理地址。

### 5 步并行分析 Pipeline

聊天记录分析采用 5 步流水线，前 4 步通过 `tokio::try_join!` 并行执行：

1. **情绪轨迹** — 逐条标注情绪类型和强度（0.0-1.0），识别转折点
2. **沟通模式** — 判断双方依恋风格（安全/焦虑/回避/混乱），分析权力结构
3. **风险识别** — 检测冷暴力、PUA、煤气灯效应、情感操控，标注严重程度和原文证据
4. **核心诉求** — 基于 NVC 框架区分表面诉求与深层需求

第 5 步依赖前 4 步结果，串行执行：

5. **建议生成** — 给出 3-5 条具体建议，包含话术改写和情景分支决策

LLM 返回 JSON，经过中文引号替换、裸换行修复等容错处理后解析。

### 卡密鉴权系统

- 卡密格式 `AMO-XXXX-XXXX-XXXX`，基于 UUID v4 生成
- 通过 Axum 中间件在请求级别验证，注入 `CardContext`
- 额度扣减使用 CTE 单次原子查询，同时完成扣减和失败原因判断
- 支持过期时间，管理员可批量生成（单条 SQL 批量 INSERT）

### 流式响应机制

情绪疏导、冲突分析、追问接口均使用 SSE 流式返回：

- 统一的 `llm_sse_stream` 辅助函数处理 LLM 流到 SSE 事件的转换
- 事件格式：`data: {"delta": "文字片段"}` / `event: done` / `event: error`
- 前端需使用 `fetch + ReadableStream`（`EventSource` 不支持自定义请求头）

### 任务生命周期管理

异步分析任务存储在内存中的 DashMap，后台定时任务每 10 分钟清理超过 30 分钟的已完成/失败任务，防止内存泄漏。

---

## 部署

```bash
# 1. 配置环境变量
cp .env.example .env
# 编辑 .env，填入数据库连接和 API Key

# 2. 启动 PostgreSQL 并创建数据库
createdb amo

# 3. 编译运行（自动执行数据库迁移）
cargo run --release

# 4. 生成卡密
curl -X POST http://localhost:3000/admin/cards \
  -H "X-Admin-Token: your_token" \
  -H "Content-Type: application/json" \
  -d '{"count": 10, "credits": 200}'
```

详细接口文档见 [API.md](./API.md)。
