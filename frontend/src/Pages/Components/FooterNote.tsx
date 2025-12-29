interface FooterNoteProps {
  text?: string;
  className?: string;
}

const DEFAULT_TEXT = "Anicargo · Recelilious · MIT License · 2025";

export default function FooterNote({ text = DEFAULT_TEXT, className = "" }: FooterNoteProps) {
  const classes = ["footer-note", className].filter(Boolean).join(" ");
  return <footer className={classes}>{text}</footer>;
}
