import "../Styles/theme.css";
import "../Styles/PageWelcome.css";
import FooterNote from "./Components/FooterNote";

export default function PageNotFound() {
  return (
    <div className="welcome-shell">
      <div className="welcome-card">
        <h1 className="welcome-title">404</h1>
        <p style={{ textAlign: "center", margin: "8px 0 0" }}>Page not found.</p>
      </div>
      <FooterNote className="welcome-footer" />
    </div>
  );
}
