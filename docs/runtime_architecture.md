# Anicargo 运行与部署整理

## 当前运行骨架

- 后端监听地址默认是 `0.0.0.0:4000`，适合局域网访问。
- 前端开发服务器默认监听 `0.0.0.0:5173`，并把 `/api` 代理到本机后端。
- 生产部署优先建议同源：
  - 前端静态文件由反向代理或同一服务域名提供
  - `/api` 继续走同源路径
- 如果前后端必须分离部署，前端通过 `VITE_API_BASE_URL` 指向真实后端地址。

## 缓存分层

- `Yuc 当前季度目录`
  - 来源：Yuc 季度页
  - 落地：`yuc_catalogs` + `yuc_catalog_entries`
  - 更新：首次请求同步，之后按 TTL 和东京时间每日状态刷新
- `Yuc 前瞻 / 特别放送目录`
  - 来源：Yuc `new` / `sp`
  - 落地：同样进入 `yuc_catalogs` + `yuc_catalog_entries`
  - 更新：按 TTL 刷新，失败时优先回退本地缓存
- `Bangumi 条目缓存`
  - 来源：Bangumi Subject/Search
  - 落地：`bangumi_subject_cache`
  - 内容：标题、封面、简介、评分、tag、状态

## 启动顺序

1. 读取配置：`命令行 > anicargo.toml > 默认值`
2. 初始化 SQLite 与迁移
3. 初始化默认管理员与下载运行参数
4. 初始化 Bangumi / Yuc / AnimeGarden / 下载引擎
5. 启动 HTTP 服务、下载同步循环、季番刷新循环、终端 TUI

## 局域网开发

- 后端：
  - `cargo run --manifest-path backend/Cargo.toml`
- 前端：
  - `npm.cmd run dev`
- 如需改代理目标：
  - 在 [frontend/web/.env.example](/E:/Dev/Web/Anicargo/frontend/web/.env.example) 基础上新增本地 `.env`
  - 设置 `VITE_DEV_PROXY_TARGET=http://<你的后端地址>:4000`

## 目前仍保留的工程方向

- 增加正式的反向代理示例（Nginx/Caddy）
- 把下载引擎、资源发现、目录缓存进一步拆成独立 service 层
- 给后台设置页补完整的持久化管理入口
