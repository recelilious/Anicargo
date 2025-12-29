import "../Styles/theme.css";
import "../Styles/PageWelcome.css";
import LogoMark from "../Icons/LogoMark";
import FooterNote from "./Components/FooterNote";

export default function PageWelcome() {
  return (
    <div className="welcome-shell">
      <div className="welcome-card">
        <div className="welcome-logo" aria-hidden="true">
          <LogoMark size={120} />
        </div>
        <h1 className="welcome-title">Anicargo</h1>
        <div className="welcome-actions">
          <a className="welcome-btn solid" href="/login">log in</a>
          <a className="welcome-btn ghost" href="/signup">sign up</a>
        </div>
      </div>
      <FooterNote className="welcome-footer" />
    </div>
  );
}
