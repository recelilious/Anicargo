import { useParams } from "react-router-dom";
import "../Styles/theme.css";
import "../Styles/PageWelcome.css";
import FooterNote from "./Components/FooterNote";

export default function PagePlayer() {
  const { id } = useParams();
  return (
    <div className="welcome-shell">
      <div className="welcome-card">
        <h1 className="welcome-title">Player</h1>
        <p style={{ textAlign: "center", margin: "8px 0 0" }}>
          {id ? `Media: ${id}` : "No media selected."}
        </p>
      </div>
      <FooterNote className="welcome-footer" />
    </div>
  );
}
