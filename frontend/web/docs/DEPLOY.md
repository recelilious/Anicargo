# Web Deployment Guide

## 1. Requirements

- Node.js 20+
- npm

## 2. Development Start

```powershell
cd .\frontend\web
npm.cmd install
npm.cmd run dev
```

The dev server listens on:

- `0.0.0.0:5173`

## 3. Environment Variables

The web client currently uses:

- `VITE_API_BASE_URL`
- `VITE_DEV_PROXY_TARGET`

See:

- `frontend/web/.env.example`

Behavior:

- If `VITE_API_BASE_URL` is empty, browser requests use relative `/api/...` paths.
- During local development, Vite proxies `/api` to `VITE_DEV_PROXY_TARGET`, which defaults to `http://127.0.0.1:4000`.

## 4. Production Build

```powershell
cd .\frontend\web
npm.cmd run build
```

Build output:

- `frontend/web/dist`

## 5. LAN Notes

For LAN testing:

- keep the backend reachable on a LAN IP
- either rely on the Vite proxy from the development host
- or set `VITE_API_BASE_URL` to the backend LAN address

Example:

```env
VITE_API_BASE_URL=http://192.168.1.10:4000
```
