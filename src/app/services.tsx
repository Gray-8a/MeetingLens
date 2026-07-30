import { isTauri } from "@tauri-apps/api/core";
import { createContext, useContext, type ReactNode } from "react";
import { meetingApi, type MeetingApi } from "../api/meeting";
import { checkSidecarHealth, type SidecarHealth } from "../lib/sidecar";
import type { TranscriptSource } from "../mock/MockTranscriptSource";

export interface AppServices {
  meetingApi: MeetingApi;
  checkSidecarHealth: () => Promise<SidecarHealth>;
  createTranscriptSource: () => Promise<TranscriptSource>;
  desktopRuntime: boolean;
}

export const defaultAppServices: AppServices = {
  meetingApi,
  checkSidecarHealth,
  createTranscriptSource: async () => {
    if (!import.meta.env.DEV) {
      throw new Error("mock_transcript_unavailable");
    }
    const { MockTranscriptSource } = await import(
      "../mock/MockTranscriptSource"
    );
    return new MockTranscriptSource();
  },
  desktopRuntime: isTauri(),
};

const ServicesContext = createContext<AppServices | null>(null);

export function ServicesProvider({
  children,
  value,
}: {
  children: ReactNode;
  value: AppServices;
}) {
  return (
    <ServicesContext.Provider value={value}>
      {children}
    </ServicesContext.Provider>
  );
}

export function useServices(): AppServices {
  const services = useContext(ServicesContext);
  if (services === null) {
    throw new Error("App services are unavailable.");
  }
  return services;
}
