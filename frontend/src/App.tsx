import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import PageWelcome from "./Pages/PageWelcome";
import PageLogin from "./Pages/PageLogin";
import PageLibrary from "./Pages/PageLibrary";
import PageSignup from "./Pages/PageSignup";
import { SessionProvider } from "./session";

export default function App() {
  return (
    <SessionProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<PageWelcome />} />
          <Route path="/login" element={<PageLogin />} />
          <Route path="/signin" element={<Navigate to="/login" replace />} />
          <Route path="/signup" element={<PageSignup />} />
          <Route path="/library" element={<PageLibrary />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </BrowserRouter>
    </SessionProvider>
  );
}
