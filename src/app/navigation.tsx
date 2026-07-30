import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type AnchorHTMLAttributes,
  type MouseEvent,
  type ReactNode,
} from "react";

export type AppRoute =
  | { name: "home" }
  | { name: "new" }
  | { name: "history" }
  | { name: "meeting"; meetingId: string }
  | { name: "live"; meetingId: string }
  | { name: "settings" };

interface NavigationValue {
  route: AppRoute;
  navigate: (path: string) => void;
}

const NavigationContext = createContext<NavigationValue | null>(null);

function currentPath(): string {
  const path = window.location.hash.replace(/^#/, "");
  return path.startsWith("/") ? path : "/";
}

export function parseRoute(path: string): AppRoute {
  if (path === "/new") return { name: "new" };
  if (path === "/history") return { name: "history" };
  if (path === "/settings") return { name: "settings" };

  const liveMatch = path.match(/^\/meetings\/([^/]+)\/live$/);
  if (liveMatch) {
    return { name: "live", meetingId: safeDecode(liveMatch[1]) };
  }

  const meetingMatch = path.match(/^\/meetings\/([^/]+)$/);
  if (meetingMatch) {
    return { name: "meeting", meetingId: safeDecode(meetingMatch[1]) };
  }

  return { name: "home" };
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function meetingPath(id: string): string {
  return `/meetings/${encodeURIComponent(id)}`;
}

export function liveMeetingPath(id: string): string {
  return `${meetingPath(id)}/live`;
}

export function NavigationProvider({ children }: { children: ReactNode }) {
  const [path, setPath] = useState(currentPath);

  useEffect(() => {
    const syncPath = () => setPath(currentPath());
    window.addEventListener("hashchange", syncPath);
    window.addEventListener("popstate", syncPath);
    return () => {
      window.removeEventListener("hashchange", syncPath);
      window.removeEventListener("popstate", syncPath);
    };
  }, []);

  const value = useMemo<NavigationValue>(
    () => ({
      route: parseRoute(path),
      navigate: (nextPath) => {
        const normalized = nextPath.startsWith("/") ? nextPath : "/";
        window.history.pushState(null, "", `#${normalized}`);
        setPath(normalized);
      },
    }),
    [path],
  );

  return (
    <NavigationContext.Provider value={value}>
      {children}
    </NavigationContext.Provider>
  );
}

export function useNavigation(): NavigationValue {
  const value = useContext(NavigationContext);
  if (value === null) {
    throw new Error("Navigation is unavailable.");
  }
  return value;
}

export function AppLink({
  to,
  onClick,
  ...props
}: AnchorHTMLAttributes<HTMLAnchorElement> & { to: string }) {
  const { navigate } = useNavigation();

  function handleClick(event: MouseEvent<HTMLAnchorElement>) {
    onClick?.(event);
    if (!event.defaultPrevented) {
      event.preventDefault();
      navigate(to);
    }
  }

  return <a {...props} href={`#${to}`} onClick={handleClick} />;
}
