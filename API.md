# Amo API 文档

> 面向前端开发者的完整接口说明，包含所有请求/响应格式、字段约束、错误处理和前端实现要点。

Base URL: `http://your-server:3000`

---

## 目录

- [通用说明](#通用说明)
- [用户端接口](#用户端接口)
  - [验证卡密](#1-验证卡密)
  - [查询余额](#2-查询剩余额度)
  - [提交聊天记录分析](#3-提交聊天记录分析)
  - [轮询分析状态](#4-轮询分析状态)
  - [分析报告追问](#5-针对分析报告追问)
  - [情绪疏导对话](#6-情绪疏导对话)
  - [冲突分析](#7-冲突分析)
  - [冲突分析追问](#8-冲突分析追问)
- [管理员端接口](#管理员端接口)
  - [批量生成卡密](#1-批量生成卡密)
  - [查看卡密列表](#2-查看所有卡密状态)
- [额度速查](#额度消耗速查)
- [前端实现要点](#前端实现要点)

---

## 通用说明

### 错误响应格式

所有接口出错时返回统一格式：

```json
{ "error": "错误描述" }
```

| HTTP 状态码 | 含义 | 前端处理建议 |
|------------|------|------------|
| 400 | 请求参数错误 | 提示用户检查输入 |
| 401 | 卡密无效或已过期 | 跳转到卡密输入页 |
| 402 | 额度不足 | 提示购买新卡密 |
| 502 | LLM 调用失败 | 提示稍后重试 |
| 500 | 服务器内部错误 | 提示稍后重试 |

### 用户端鉴权

除 `POST /cards/verify` 外，所有用户接口需在请求头携带卡密：

```
Authorization: Bearer AMO-XXXX-XXXX-XXXX
```

卡密存储在前端 `localStorage`，key 建议用 `amo_card_code`。

### 管理员端鉴权

所有 `/admin/*` 接口需在请求头携带 Admin Token：

```
X-Admin-Token: your_admin_token
```

### SSE 流式响应

情绪疏导、冲突分析、冲突分析追问、分析报告追问四个接口返回 `text/event-stream`。

**事件格式：**

```
// 正常内容片段（无 event 字段）
data: {"delta": "文字片段"}

// 结束
event: done
data:

// 错误
event: error
data: 错误描述
```

**推荐实现方式（fetch + ReadableStream，支持携带请求头）：**

```javascript
async function streamRequest(url, body, cardCode, onDelta, onDone, onError) {
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${cardCode}`,
    },
    body: JSON.stringify(body),
  })

  if (!response.ok) {
    const err = await response.json()
    onError(err.error)
    return
  }

  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() // 保留未完整的行

    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const data = line.slice(6)
        if (!data) continue
        try {
          const { delta } = JSON.parse(data)
          if (delta) onDelta(delta)
        } catch {}
      }
      if (line === 'event: done') {
        onDone()
        return
      }
      if (line === 'event: error') {
        // 下一行 data 是错误信息
      }
    }
  }
}
```

> 注意：`EventSource` 不支持自定义请求头，无法携带 `Authorization`，必须用 `fetch` 实现 SSE。

---

## 用户端接口

---

### 1. 验证卡密

验证卡密是否有效，不消耗额度。用于用户首次输入卡密时的校验。

```
POST /cards/verify
Content-Type: application/json
```

**请求体**

```json
{ "code": "AMO-A1B2-C3D4-E5F6" }
```

**响应 — 有效**

```json
{
  "valid": true,
  "credits": 200,
  "total": 200
}
```

**响应 — 无效或已过期**

```json
{ "valid": false }
```

**前端逻辑：**
1. 用户输入卡密后调用此接口
2. `valid: true` → 将卡密存入 `localStorage`，跳转主界面
3. `valid: false` → 提示"卡密无效或已过期"

---

### 2. 查询剩余额度

```
GET /cards/balance
Authorization: Bearer AMO-XXXX-XXXX-XXXX
```

**响应**

```json
{ "credits": 180 }
```

**前端逻辑：** 每次进入主界面时调用，将余额展示在页面顶部。

---

### 3. 提交聊天记录分析

提交后立即返回 `task_id`，分析在后台异步执行（约 1-2 分钟），需轮询状态。消耗 **20 credits**。

```
POST /analysis
Authorization: Bearer AMO-XXXX-XXXX-XXXX
Content-Type: application/json
```

**请求体**

```json
{
  "background": {
    "self_info": {
      "name": "小A",
      "age": 25,
      "notes": "性格比较敏感，有焦虑倾向"
    },
    "partner_info": {
      "name": "小B",
      "age": 27,
      "notes": "话不多，遇事喜欢冷处理"
    },
    "relationship": "恋爱中，交往8个月，异地"
  },
  "messages": [
    { "speaker": "self",    "text": "你今天怎么了" },
    { "speaker": "partner", "text": "没事" },
    { "speaker": "self",    "text": "真的没事吗，感觉你很冷淡" },
    { "speaker": "partner", "text": "说没事就没事，你想太多了" }
  ]
}
```

| 字段 | 类型 | 必填 | 约束 |
|------|------|------|------|
| `background` | object | 否 | 提供后分析更准确 |
| `background.self_info.name` | string | 否 | - |
| `background.self_info.age` | number | 否 | - |
| `background.self_info.notes` | string | 否 | 补充信息，如性格、家庭背景等 |
| `background.partner_info` | object | 否 | 同 self_info |
| `background.relationship` | string | 否 | 关系现状自由描述 |
| `messages` | array | 是 | 1-100 条 |
| `messages[].speaker` | string | 是 | 只能是 `"self"` 或 `"partner"` |
| `messages[].text` | string | 是 | 最多 500 字 |

**响应**

```json
{
  "task_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "processing",
  "credits_used": 20
}
```

**前端逻辑：**
1. 提交后将 `task_id` 存入组件状态
2. 立即开始轮询（见接口 4）
3. 展示"分析中"加载状态

---

### 4. 轮询分析状态

```
GET /analysis/:task_id
Authorization: Bearer AMO-XXXX-XXXX-XXXX
```

**响应 — 处理中**

```json
{ "status": "processing" }
```

**响应 — 完成**

```json
{
  "status": "done",
  "report": {
    "emotion_trajectory": {
      "segments": [
        { "index": 0, "speaker": "self",    "emotion": "焦虑",  "intensity": 0.6 },
        { "index": 1, "speaker": "partner", "emotion": "冷漠",  "intensity": 0.7 },
        { "index": 2, "speaker": "self",    "emotion": "委屈",  "intensity": 0.7 },
        { "index": 3, "speaker": "partner", "emotion": "愤怒",  "intensity": 0.85 }
      ],
      "turning_points": [
        { "index": 3, "description": "对方语气变得强硬，情绪从冷漠升级为愤怒" }
      ],
      "summary": "一方试图关心但遭遇冷漠，持续追问后对方情绪骤然激化"
    },
    "communication_patterns": {
      "self_attachment_style": "焦虑型",
      "partner_attachment_style": "回避型",
      "power_dynamic": "对方通过情感撤退控制互动节奏，你处于追逐和试探的位置",
      "failure_modes": ["追逃模式", "情感隔离"],
      "summary": "典型的焦虑-回避组合，你越靠近对方越往后退"
    },
    "risk_flags": [
      {
        "flag_type": "cold_violence",
        "severity": "medium",
        "evidence_indices": [1, 3, 5],
        "evidence_text": "没事 / 说没事就没事 / 不需要你关心，烦死了",
        "explanation": "对方用简短冷淡的回应持续拒绝沟通，最终表达厌烦，这种情感撤回会让关心的一方觉得表达爱是一种错"
      },
      {
        "flag_type": "gaslighting",
        "severity": "low",
        "evidence_indices": [3],
        "evidence_text": "说没事就没事，你想太多了",
        "explanation": "否定对方的感知，让其怀疑自己的判断力"
      }
    ],
    "core_needs": {
      "self_surface": "想确认对方是否有情绪",
      "self_deep": "需要连接和确定性，渴望被信任、被允许进入对方的情绪世界",
      "partner_surface": "拒绝关心，说没事",
      "partner_deep": "需要自主和安全感，可能不想被追问，需要空间处理情绪"
    },
    "suggestions": [
      {
        "context": "刚被推开之后，不知道该怎么接",
        "original": "我只是关心你",
        "rewrite": "好，我不问了。你想说的时候我在。",
        "rationale": "停止追问给空间，同时留门没关死，对方反而更可能主动开口"
      },
      {
        "context": "冷静后想重新开启对话",
        "original": "你今天怎么了",
        "rewrite": "最近感觉我们有点远，不知道是不是我的错觉。你有空的时候聊聊？",
        "rationale": "把焦点从'你怎么了'转移到'我们之间'，不会让对方感到被审问"
      }
    ]
  }
}
```

**响应 — 失败**

```json
{
  "status": "failed",
  "error": "LLM 调用失败: ..."
}
```

**report 字段说明：**

| 字段 | 说明 |
|------|------|
| `emotion_trajectory.segments[].speaker` | `"self"` 或 `"partner"` |
| `emotion_trajectory.segments[].intensity` | 0.0-1.0，越高情绪越强烈 |
| `risk_flags[].flag_type` | `cold_violence` / `pua` / `gaslighting` / `manipulation` |
| `risk_flags[].severity` | `low` / `medium` / `high` |
| `risk_flags[].evidence_indices` | 对应 messages 数组的下标 |
| `suggestions[].original` | 原始话术，可能为 `null` |

**前端轮询逻辑：**

```javascript
async function pollAnalysis(taskId, cardCode, onDone, onError) {
  const maxAttempts = 40  // 最多等 2 分钟
  let attempts = 0

  const poll = async () => {
    attempts++
    if (attempts > maxAttempts) {
      onError('分析超时，请重试')
      return
    }

    const res = await fetch(`/analysis/${taskId}`, {
      headers: { 'Authorization': `Bearer ${cardCode}` }
    })
    const data = await res.json()

    if (data.status === 'done') {
      onDone(data.report)
    } else if (data.status === 'failed') {
      onError(data.error)
    } else {
      setTimeout(poll, 3000)  // 3 秒后再试
    }
  }

  poll()
}
```

---

### 5. 针对分析报告追问

对已完成的分析报告进行追问，流式返回。消耗 **3 credits / 次**。

```
POST /analysis/followup
Authorization: Bearer AMO-XXXX-XXXX-XXXX
Content-Type: application/json
```

**请求体**

```json
{
  "question": "为什么说对方是回避型依恋？有什么具体表现？",
  "report": { "...上一步返回的完整 report 对象..." }
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `question` | string | 是 | 追问内容 |
| `report` | object | 是 | 完整的 report 对象，直接传 `/analysis/:id` 返回的 report 字段 |

**响应** — SSE 流式，见通用说明

**前端逻辑：** 报告展示页底部提供输入框，用户输入问题后调用此接口，将流式回复展示在报告下方。

---

### 6. 情绪疏导对话

多轮对话，AI 扮演知心朋友进行情绪疏导。消耗 **2 credits / 轮**。

```
POST /emotional/chat
Authorization: Bearer AMO-XXXX-XXXX-XXXX
Content-Type: application/json
```

**请求体**

```json
{
  "message": "他今天又不回我消息了，我是不是太敏感了",
  "history": [
    { "role": "user",      "content": "我男朋友最近对我越来越冷淡" },
    { "role": "assistant", "content": "听起来你最近压力挺大的，能说说具体发生了什么吗？" }
  ]
}
```

| 字段 | 类型 | 必填 | 约束 |
|------|------|------|------|
| `message` | string | 是 | 最多 1000 字 |
| `history` | array | 否 | 首轮传空数组 `[]` 或不传 |
| `history[].role` | string | 是 | `"user"` 或 `"assistant"` |
| `history[].content` | string | 是 | 消息内容 |

**响应** — SSE 流式，见通用说明

**前端多轮对话维护逻辑：**

```javascript
// 组件状态
const history = []

async function sendMessage(message, cardCode) {
  // 1. 发起请求
  await streamRequest(
    '/emotional/chat',
    { message, history },
    cardCode,
    (delta) => appendToLastMessage(delta),  // 流式拼接
    () => {},
    (err) => showError(err)
  )

  // 2. 请求完成后，将本轮对话追加到 history
  history.push({ role: 'user', content: message })
  history.push({ role: 'assistant', content: getLastMessageContent() })
}
```

---

### 7. 冲突分析

描述一次争吵或冲突，AI 分析双方诉求并给出情景分支决策。消耗 **8 credits**。

```
POST /conflict/analyze
Authorization: Bearer AMO-XXXX-XXXX-XXXX
Content-Type: application/json
```

**请求体**

```json
{
  "description": "昨晚我们因为他玩游戏吵架了。我说他每天回家就玩游戏，从来不陪我。他说我太粘人，他需要自己的时间。我说那你当初为什么要在一起，他就不说话了，冷战到现在。",
  "background": "在一起两年，同居半年，他平时工作压力大"
}
```

| 字段 | 类型 | 必填 | 约束 |
|------|------|------|------|
| `description` | string | 是 | 最多 2000 字 |
| `background` | string | 否 | 背景信息，提供后分析更准确 |

**响应** — SSE 流式，见通用说明

---

### 8. 冲突分析追问

对冲突分析结果进行追问，流式返回。消耗 **5 credits / 次**。

```
POST /conflict/followup
Authorization: Bearer AMO-XXXX-XXXX-XXXX
Content-Type: application/json
```

**请求体**

```json
{
  "question": "你说的冷暴力具体指哪些表现？我该怎么应对？",
  "analysis": "上一次 /conflict/analyze 返回的完整文本（前端拼接 SSE delta 后的完整字符串）",
  "description": "（可选）原始冲突描述，提供后上下文更完整"
}
```

| 字段 | 类型 | 必填 | 约束 |
|------|------|------|------|
| `question` | string | 是 | 最多 1000 字 |
| `analysis` | string | 是 | 上次冲突分析的完整回复文本 |
| `description` | string | 否 | 原始冲突描述 |

**响应** — SSE 流式，见通用说明

**前端逻辑：** 冲突分析结果展示页底部提供输入框，用户输入追问后调用此接口。前端需在冲突分析 SSE 流结束后，将所有 delta 拼接保存为完整文本，作为 `analysis` 字段传入。

---

## 管理员端接口

---

### 1. 批量生成卡密

```
POST /admin/cards
X-Admin-Token: your_admin_token
Content-Type: application/json
```

**请求体**

```json
{
  "count": 10,
  "credits": 200,
  "expires_at": null
}
```

| 字段 | 类型 | 必填 | 约束 |
|------|------|------|------|
| `count` | number | 是 | 1-100 |
| `credits` | number | 是 | 大于 0 |
| `expires_at` | number\|null | 否 | Unix 时间戳，`null` 表示永不过期 |

**额度定价参考**

| 售价 | 建议 credits | 可用次数参考 |
|------|-------------|-------------|
| 9.9 元 | 30 | 聊天记录分析 ×1 + 冲突分析 ×1 + 情绪疏导 ×5 |
| 29.9 元 | 150 | 聊天记录分析 ×5 + 冲突分析 ×3 + 情绪疏导 ×15 |
| 59.9 元 | 400 | 聊天记录分析 ×14 + 冲突分析 ×6 + 情绪疏导 ×30 |

**响应**

```json
{
  "count": 10,
  "credits_per_card": 200,
  "expires_at": null,
  "codes": [
    "AMO-A1B2-C3D4-E5F6",
    "AMO-G7H8-I9J0-K1L2"
  ]
}
```

---

### 2. 查看所有卡密状态

```
GET /admin/cards
X-Admin-Token: your_admin_token
```

**响应**

```json
{
  "total": 25,
  "cards": [
    {
      "code": "AMO-A1B2-C3D4-E5F6",
      "credits": 180,
      "total": 200,
      "used": 20,
      "created_at": 1718000000,
      "expires_at": null
    }
  ]
}
```

| 字段 | 说明 |
|------|------|
| `credits` | 剩余额度 |
| `total` | 初始总额度 |
| `used` | 已消耗（`total - credits`）|
| `created_at` | 创建时间（Unix 时间戳）|
| `expires_at` | 过期时间，`null` 表示永不过期 |

---

## 额度消耗速查

| 功能 | 消耗 credits |
|------|-------------|
| 聊天记录分析（完整） | 20 |
| 分析报告追问（每次） | 3 |
| 情绪疏导对话（每轮） | 2 |
| 冲突分析 | 8 |
| 冲突分析追问（每次） | 5 |
| 验证卡密 / 查询余额 | 0 |

---

## 前端实现要点

### 页面结构建议

**用户端（4个页面）：**

1. **卡密输入页** — 输入框 + 验证按钮，验证成功后存 localStorage 跳转主页
2. **主页** — 展示余额，提供三个功能入口（聊天记录分析 / 情绪疏导 / 冲突分析）
3. **聊天记录分析页** — 分两步：① 填写背景信息 + 逐条输入对话 → ② 展示分析报告 + 追问输入框
4. **情绪疏导 / 冲突分析页** — 聊天气泡界面，流式展示 AI 回复

**管理员端（1个页面）：**

1. **卡密管理页** — 生成卡密表单 + 卡密列表表格

### 状态管理

```javascript
// localStorage 存储
localStorage.setItem('amo_card_code', 'AMO-XXXX-XXXX-XXXX')
localStorage.getItem('amo_card_code')
localStorage.removeItem('amo_card_code')  // 401 时清除，跳转卡密输入页

// 全局拦截 401
async function request(url, options = {}) {
  const cardCode = localStorage.getItem('amo_card_code')
  const res = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${cardCode}`,
      ...options.headers,
    }
  })
  if (res.status === 401) {
    localStorage.removeItem('amo_card_code')
    window.location.href = '/'  // 跳转卡密输入页
    return
  }
  return res
}
```

### 聊天记录分析页交互流程

```
用户填写背景信息（可选）
    ↓
用户逐条输入对话（self/partner 切换）
    ↓
点击"开始分析"→ POST /analysis → 获得 task_id
    ↓
展示加载动画，每 3 秒轮询 GET /analysis/:task_id
    ↓
status = done → 渲染报告（5个模块）
    ↓
用户可在报告下方输入追问 → POST /analysis/followup（SSE）
```

### 报告展示模块

分析报告包含 5 个模块，建议分卡片展示：

| 模块 | 字段 | 展示建议 |
|------|------|---------|
| 情绪轨迹 | `emotion_trajectory` | 时间轴或折线图，标注转折点 |
| 沟通模式 | `communication_patterns` | 依恋风格标签 + 文字说明 |
| 风险标注 | `risk_flags` | 红/橙/黄色标签，点击展开证据和解释 |
| 核心诉求 | `core_needs` | 双栏对比（我 vs 对方），表面/深层各一行 |
| 建议 | `suggestions` | 卡片列表，原话 → 改写，附理由 |

### 风险等级颜色

```javascript
const severityColor = {
  low:    '#F59E0B',  // 黄色
  medium: '#EF4444',  // 红色
  high:   '#7F1D1D',  // 深红
}

const flagTypeLabel = {
  cold_violence: '冷暴力',
  pua:           'PUA',
  gaslighting:   '煤气灯效应',
  manipulation:  '情感操控',
}
```

### 注意事项

1. **SSE 必须用 fetch**，不能用 `EventSource`（不支持自定义请求头）
2. **情绪疏导的 history 由前端维护**，每轮请求后手动追加
3. **分析任务存内存**，服务重启后 task_id 失效，需重新提交
4. **额度不足（402）** 时提示用户购买新卡密，不要自动跳转
5. **intensity 字段**是 float，展示时建议乘以 100 转为百分比或用进度条
