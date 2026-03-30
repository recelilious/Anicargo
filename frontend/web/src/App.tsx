import { Route, Routes } from "react-router-dom";

import { AppShell } from "./components/AppShell";
import { AdminPage } from "./pages/AdminPage";
import { HistoryPage } from "./pages/HistoryPage";
import { ResourcesPage } from "./pages/ResourcesPage";
import { SearchPage } from "./pages/SearchPage";
import { SeasonPage } from "./pages/SeasonPage";
import { SettingsPage } from "./pages/SettingsPage";
import { SubscriptionsPage } from "./pages/SubscriptionsPage";
import { SubjectPage } from "./pages/SubjectPage";
import { WatchPage } from "./pages/WatchPage";
import { PreviewPage } from "./pages/YucCatalogPage";

export default function App() {
  return (
    <Routes>
      <Route path="/admin" element={<AdminPage />} />
      <Route element={<AppShell />}>
        <Route path="/" element={<SeasonPage />} />
        <Route path="/search" element={<SearchPage />} />
        <Route path="/subscriptions" element={<SubscriptionsPage />} />
        <Route path="/preview" element={<PreviewPage />} />
        <Route path="/resources" element={<ResourcesPage />} />
        <Route path="/history" element={<HistoryPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/title/:subjectId" element={<SubjectPage />} />
        <Route path="/watch/:subjectId/:episodeId" element={<WatchPage />} />
      </Route>
    </Routes>
  );
}
