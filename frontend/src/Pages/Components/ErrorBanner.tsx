interface ErrorBannerProps {
  message: string;
  className?: string;
}

export default function ErrorBanner({ message, className = "" }: ErrorBannerProps) {
  const classes = ["error-banner", className].filter(Boolean).join(" ");
  return (
    <div role="alert" className={classes}>
      {message}
    </div>
  );
}
