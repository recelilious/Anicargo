# Web Architecture

## 1. Entry Points

- `frontend/web/src/main.tsx`
- `frontend/web/src/App.tsx`

The app uses `react-router-dom` and a shared shell layout for all routes. The management view is only exposed when the current logged-in account has admin privileges.

## 2. Route Map

- `/` -> `SeasonPage.tsx`
- `/search` -> `SearchPage.tsx`
- `/subscriptions` -> `SubscriptionsPage.tsx`
- `/preview` -> `YucCatalogPage.tsx`
- `/special` -> `YucCatalogPage.tsx`
- `/resources` -> `ResourcesPage.tsx`
- `/history` -> `HistoryPage.tsx`
- `/settings` -> `SettingsPage.tsx`
- `/title/:subjectId` -> `SubjectPage.tsx`
- `/watch/:subjectId/:episodeId` -> `WatchPage.tsx`
- `/manage` -> `AdminPage.tsx`

## 3. Layout And Shared UI

Main layout:

- `frontend/web/src/components/AppShell.tsx`

Reusable cards and playback UI:

- `frontend/web/src/components/SubjectCard.tsx`
- `frontend/web/src/components/EpisodeCard.tsx`
- `frontend/web/src/components/AnicargoPlayer.tsx`

## 4. API Boundary

All HTTP calls are centralized in:

- `frontend/web/src/api.ts`

This file owns:

- base URL resolution
- header injection
- auth/device token plumbing
- endpoint wrappers used by pages

## 5. Session, Appearance, And Navigation State

- `frontend/web/src/session.tsx`
- `frontend/web/src/appearance.tsx`
- `frontend/web/src/navigation.ts`
- `frontend/web/src/theme.ts`

These modules handle:

- device identity bootstrap
- optional user session persistence
- appearance mode and deep-night settings
- return-to-previous-position navigation behavior

## 6. Page Responsibilities

### Season and Yuc catalogs

- `SeasonPage.tsx`
- `YucCatalogPage.tsx`

### Search and collections

- `SearchPage.tsx`
- `SubscriptionsPage.tsx`
- `ResourcesPage.tsx`
- `HistoryPage.tsx`

### Detail and playback

- `SubjectPage.tsx`
- `WatchPage.tsx`

### Admin

- `AdminPage.tsx`

## 7. Streaming Model

The web player currently consumes direct HTTP Range streaming from the backend. It does not depend on HLS or DASH today.
