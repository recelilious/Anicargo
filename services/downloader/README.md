# Anicargo Downloader

`anicargo-downloader` 是给 Anicargo 准备的独立下载核心服务。它可以单独监听 HTTP API，也可以在后续作为库被主后端直接调用。

当前已具备的基础能力：

- 创建、查看、暂停、恢复、删除下载任务
- 创建、查看、暂停、恢复、删除做种任务
- 通过磁力链接或 `.torrent` 文件只读取种子元信息
- 按优先级 + 创建时间排队
- 最大同时下载数、最大同时做种数限制
- 全局下载/上传限速
- 单任务下载/上传限速
- 无进度超时、总超时
- 实时查看任务状态、速率、Peer、进度

## 启动

```powershell
cargo run --manifest-path services/downloader/Cargo.toml -- `
  --config services/downloader/config/downloader.example.toml
```

默认监听：

- `0.0.0.0:4010`

## 主要接口

- `GET /api/health`
- `GET /api/v1/runtime`
- `PATCH /api/v1/settings`
- `POST /api/v1/inspect`
- `GET /api/v1/tasks`
- `POST /api/v1/tasks`
- `GET /api/v1/tasks/{task_id}`
- `PATCH /api/v1/tasks/{task_id}`
- `POST /api/v1/tasks/{task_id}/pause`
- `POST /api/v1/tasks/{task_id}/resume`
- `DELETE /api/v1/tasks/{task_id}?delete_files=true`
- `GET /api/v1/downloads`
- `GET /api/v1/seeds`

## 手动测试

项目里附带了一个 PowerShell 测试脚本：

```powershell
pwsh scripts/test-downloader-service.ps1 `
  -BaseUrl http://127.0.0.1:4010 `
  -TorrentFile backend/runtime/media/_rqbit/session/beffa75ad2eda16e5409572ffada51131e8bf198.torrent
```

这个脚本会顺序验证：

- 健康检查
- 读取运行参数
- 探测 `.torrent` 元信息
- 创建暂停状态的下载任务
- 恢复任务
- 查看任务列表
- 暂停任务
- 删除任务

## 配置优先级

- 启动项
- 配置文件
- 默认值

示例配置见：

- [downloader.example.toml](/E:/Dev/Web/Anicargo/services/downloader/config/downloader.example.toml)
