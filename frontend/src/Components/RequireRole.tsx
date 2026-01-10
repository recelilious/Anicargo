import type { ReactNode } from "react";
import { useSession } from "../session";
import PageNotFound from "../Pages/PageNotFound";

interface RequireRoleProps {
  minLevel: number;
  children: ReactNode;
}

export default function RequireRole({ minLevel, children }: RequireRoleProps) {
  const { session } = useSession();
  if (!session) {
    return <PageNotFound />;
  }
  if (session.roleLevel < minLevel) {
    return <PageNotFound />;
  }
  return <>{children}</>;
}
