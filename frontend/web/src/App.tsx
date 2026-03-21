import { Route, Routes } from "react-router-dom";

import { AppShell } from "./components/AppShell";
import { AdminPage } from "./pages/AdminPage";
import { ResourcesPage } from "./pages/ResourcesPage";
import { SearchPage } from "./pages/SearchPage";
import { SeasonPage } from "./pages/SeasonPage";
import { SettingsPage } from "./pages/SettingsPage";
import { SubjectPage } from "./pages/SubjectPage";
import { WatchPage } from "./pages/WatchPage";

export default function App() {
  return (
    <Routes>
      <Route path="/admin" element={<AdminPage />} />
      <Route element={<AppShell />}>
        <Route path="/" element={<SeasonPage />} />
        <Route path="/search" element={<SearchPage />} />
        <Route path="/resources" element={<ResourcesPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/title/:subjectId" element={<SubjectPage />} />
        <Route path="/watch/:subjectId/:episodeId" element={<WatchPage />} />
      </Route>
    </Routes>
  );
}
