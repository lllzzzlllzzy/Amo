# 本地测试指南

## 第一步：准备环境

**1.1 创建数据库**

```bash
psql -U postgres
CREATE DATABASE amo;
\q
```

**1.2 创建 `.env` 文件**

```bash
cp .env.example .env
```

编辑 `.env`，填入实际值：

```env
DATABASE_URL=postgres://postgres:你的密码@localhost:5432/amo
ADMIN_TOKEN=Duckie0113
LLM_PROVIDER=anthropic
ANTHROPIC_AUTH_TOKEN=你的token
ANTHROPIC_BASE_URL=https://xuedingtoken.com
ANTHROPIC_SMART_MODEL=claude-3-5-sonnet-20241022
ANTHROPIC_FAST_MODEL=claude-3-5-haiku-20241022
HOST=0.0.0.0
PORT=3000
```

> 注意：模型名称必须使用带日期的完整格式（如 `claude-3-5-haiku-20241022`），不能用简短别名（如 `claude-haiku-4-5`），否则第三方代理会报 `No available accounts`。

---

## 第二步：启动服务

```bash
cargo run
```

看到以下输出说明启动成功：

```
INFO amo: Amo 后端启动，监听 0.0.0.0:3000
```

调试模式（可看到详细日志）：

```bash
RUST_LOG=amo=debug cargo run
```

---

## 第三步：测试管理员接口

**3.1 生成卡密**

```bash
curl -s -X POST http://127.0.0.1:3000/admin/cards \
  -H 'X-Admin-Token: Duckie0113' \
  -H 'Content-Type: application/json' \
  -d '{"count": 3, "credits": 200}'
```

响应：

```json
{
  "codes": [
    "AMO-F78D-74B8-6383",
    "AMO-9FAD-9425-A300",
    "AMO-2A2E-AB9D-9AA1"
  ],
  "count": 3,
  "credits_per_card": 200,
  "expires_at": null
}
```

**把其中一个卡密记下来，后续步骤用 `$CODE` 代替。**

**3.2 查看卡密列表**

```bash
curl -s http://127.0.0.1:3000/admin/cards \
  -H 'X-Admin-Token: Duckie0113'
```

---

## 第四步：测试用户接口

**4.1 验证卡密**

```bash
curl -s -X POST http://127.0.0.1:3000/cards/verify \
  -H 'Content-Type: application/json' \
  -d '{"code": "AMO-F78D-74B8-6383"}'
```

响应：

```json
{ "valid": true, "credits": 200, "total": 200 }
```

**4.2 查询余额**

```bash
curl -s http://127.0.0.1:3000/cards/balance \
  -H 'Authorization: Bearer AMO-F78D-74B8-6383'
```

响应：

```json
{ "credits": 200 }
```

**4.3 测试无效卡密**

```bash
curl -s http://127.0.0.1:3000/cards/balance \
  -H 'Authorization: Bearer AMO-FAKE-FAKE-FAKE'
```

响应：HTTP 401，`{"error": "卡密无效或已过期"}`

---

## 第五步：测试情绪疏导（SSE）

```bash
curl -s -N -X POST http://127.0.0.1:3000/emotional/chat \
  -H 'Authorization: Bearer AMO-F78D-74B8-6383' \
  -H 'Content-Type: application/json' \
  -d '{"message": "他今天又不回我消息了，我是不是太敏感了", "history": []}'
```

响应（流式）：

```
data: {"delta":"我能"}
data: {"delta":"理解你现"}
data: {"delta":"在的感受..."}
...
event: done
data:
```

---

## 第六步：测试冲突分析（SSE）

```bash
curl -s -N -X POST http://127.0.0.1:3000/conflict/analyze \
  -H 'Authorization: Bearer AMO-F78D-74B8-6383' \
  -H 'Content-Type: application/json' \
  -d '{"description": "昨晚因为他玩游戏吵架了，我说他不陪我，他说我太粘人，然后冷战到现在", "background": "在一起两年，同居半年"}'
```

响应（流式）：AI 会分析双方诉求，识别冲突升级节点，给出情景分支建议。

---

## 第七步：测试聊天记录分析（异步）

**7.1 提交分析**

```bash
curl -s -X POST http://127.0.0.1:3000/analysis \
  -H 'Authorization: Bearer AMO-F78D-74B8-6383' \
  -H 'Content-Type: application/json' \
  -d '{
    "background": {
      "self_info": {"name": "小A", "age": 25},
      "partner_info": {"name": "小B", "age": 27},
      "relationship": "恋爱中，交往8个月"
    },
    "messages": [
      {"speaker": "self",    "text": "你今天怎么了"},
      {"speaker": "partner", "text": "没事"},
      {"speaker": "self",    "text": "真的没事吗"},
      {"speaker": "partner", "text": "说没事就没事"}
    ]
  }'
```

响应：

```json
{
  "credits_used": 20,
  "status": "processing",
  "task_id": "ef5f9561-d015-473b-b705-9310d7ee0284"
}
```

**7.2 轮询结果**

每隔 20-30 秒执行一次，通常 1-2 分钟内完成：

```bash
curl -s http://127.0.0.1:3000/analysis/ef5f9561-d015-473b-b705-9310d7ee0284 \
  -H 'Authorization: Bearer AMO-F78D-74B8-6383'
```

完成后响应示例：

```json
{
  "status": "done",
  "report": {
    "emotion_trajectory": {
      "segments": [
        {"emotion": "焦虑", "index": 0, "intensity": 0.6, "speaker": "self"},
        {"emotion": "冷漠", "index": 1, "intensity": 0.7, "speaker": "partner"},
        {"emotion": "焦虑", "index": 2, "intensity": 0.7, "speaker": "self"},
        {"emotion": "愤怒", "index": 3, "intensity": 0.8, "speaker": "partner"}
      ],
      "turning_points": [
        {"description": "对方用冷漠回应关心，开始筑起防御", "index": 1},
        {"description": "对方语气变得强硬带有攻击性，情绪从冷漠升级为愤怒", "index": 3}
      ],
      "summary": "一方试图关心但遭遇冷漠回应，持续追问后对方情绪从冷漠升级为愤怒防御"
    },
    "communication_patterns": {
      "self_attachment_style": "焦虑型",
      "partner_attachment_style": "回避型",
      "failure_modes": ["追逃模式", "情感隔离"],
      "power_dynamic": "对方通过情感撤退控制互动节奏，你处于追逐和试探的位置",
      "summary": "典型的追逃循环，你越想知道对方怎么了，对方就越往后退"
    },
    "risk_flags": [
      {
        "flag_type": "cold_violence",
        "severity": "low",
        "evidence_indices": [1, 3],
        "evidence_text": "没事 / 说没事就没事",
        "explanation": "对方用简短冷淡的回应拒绝沟通，暂定低风险，对话太短无法确认是否为持续模式"
      }
    ],
    "core_needs": {
      "self_surface": "询问对方的状态，想知道对方是否有问题",
      "self_deep": "需要连接和确定性，渴望被信任、被允许进入对方的情绪世界",
      "partner_surface": "否认有问题，拒绝深入交流",
      "partner_deep": "需要自主和安全感，可能不想被追问，需要自己的空间处理情绪"
    },
    "suggestions": [
      {
        "context": "当对方说'没事'时，你想确认但又怕惹毛TA",
        "original": "真的没事吗",
        "rewrite": "好的，我在。如果想聊了随时找我",
        "rationale": "停止追问，给空间。传递'我关心你但不逼你'，对方反而更可能主动开口"
      }
    ]
  }
}
```

**7.3 测试追问**

```bash
curl -s -N -X POST http://127.0.0.1:3000/analysis/followup \
  -H 'Authorization: Bearer AMO-F78D-74B8-6383' \
  -H 'Content-Type: application/json' \
  -d '{"question": "为什么说对方是回避型依恋？", "report": {...上一步返回的完整 report 对象...}}'
```

**7.4 确认额度扣减**

```bash
curl -s http://127.0.0.1:3000/cards/balance \
  -H 'Authorization: Bearer AMO-F78D-74B8-6383'
```

每次完整分析扣 20 credits。

---

## 常见问题

| 现象 | 原因 | 解决 |
|------|------|------|
| `No available accounts` | 模型名称格式错误 | 使用带日期的完整格式，如 `claude-3-5-haiku-20241022` |
| `ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN must be set` | .env 未加载 | 确认 .env 在项目根目录 |
| `Connection refused` 连不上数据库 | PG 未启动或密码错误 | 检查 `DATABASE_URL` |
| `额度不足` | 卡密 credits 耗尽 | 调用 `POST /admin/cards` 生成新卡密 |
| 分析一直 `processing` | 流水线耗时较长 | 正常，通常 1-2 分钟，耐心等待 |
| `task_id` 查不到 | 服务重启后内存清空 | 任务状态存在内存中，重启后丢失，重新提交即可 |
