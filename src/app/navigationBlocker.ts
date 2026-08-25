import { useEffect, useRef } from "react";
import { useNavigation } from "./navigation";

export function useNavigationBlocker(
  shouldBlock: () => boolean,
  onBlocked: (path: string) => void,
) {
  const { registerBlocker } = useNavigation();
  const shouldBlockRef = useRef(shouldBlock);
  const onBlockedRef = useRef(onBlocked);

  useEffect(() => {
    shouldBlockRef.current = shouldBlock;
    onBlockedRef.current = onBlocked;
  }, [onBlocked, shouldBlock]);

  useEffect(
    () =>
      registerBlocker({
        shouldBlock: () => shouldBlockRef.current(),
        onBlocked: (path) => onBlockedRef.current(path),
      }),
    [registerBlocker],
  );
}
