# Anicargo Downloader

`anicargo-downloader` 是给 Anicargo 准备的独立下载核心服务。

它现在支持两种使用方式：

- 独立 HTTP 服务
- 作为 Rust 库嵌入到别的程序里直接调用

当前已具备的基础能力：

- 创建、查看、暂停、恢复、删除下载任务
- 创建、查看、暂停、恢复、删除做种任务
- 通过磁力链接或 `.torrent` 文件只读取种子元信息
- 同 `info_hash` 任务去重，避免重复创建同一个下载
- 按优先级加创建时间排队
- 最大同时下载数、最大同时做种数限制
- 全局下载/上传限速
- 单任务下载/上传限速
- 无进度超时、总超时
- 实时查看任务状态、速率、Peer、进度
- 默认下载目录 + 单任务输出目录覆盖

## 启动

```powershell
cargo run --manifest-path services/downloader/Cargo.toml -- `
  --config services/downloader/config/downloader.example.toml
```

默认监听：

- `0.0.0.0:4010`

## 启动参数

- `--config`
- `--listen`
- `--runtime-root`
- `--default-output-dir`
- `--max-concurrent-downloads`
- `--max-concurrent-seeds`
- `--global-download-limit-mb`
- `--global-upload-limit-mb`
- `--priority-decay`
- `--stall-timeout-secs`
- `--total-timeout-secs`
- `--scheduler-interval-secs`

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

## 作为库使用

不需要开端口时，可以直接在 Rust 代码里调用：

```rust
use anicargo_downloader::{DownloaderConfig, start_embedded};

let runtime = start_embedded(DownloaderConfig::default())?;
let service = runtime.service();

let runtime_info = service.runtime_overview().await?;
println!("{:?}", runtime_info.settings);
```

如果需要 HTTP 服务，再把 `service` 交给 `build_router` 即可。

## 手动测试脚本

基础冒烟脚本：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/test-downloader-service.ps1 `
  -BaseUrl http://127.0.0.1:4010 `
  -TorrentFile backend/runtime/media/_rqbit/session/beffa75ad2eda16e5409572ffada51131e8bf198.torrent
```

长跑混合下载脚本：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/test-downloader-service-longrun.ps1 `
  -BaseUrl http://127.0.0.1:4010
```
