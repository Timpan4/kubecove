import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { router } from "./app/router";
import { TooltipProvider } from "@/components/ui/tooltip";
import { queryRetry } from "./lib/query-retry";
import { setDiagnosticsEnabled } from "./lib/diagnostics";
import { useSettingsState } from "./lib/settings";

document.documentElement.classList.add("dark");
setDiagnosticsEnabled(useSettingsState.getState().debugModeEnabled);

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: queryRetry,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <TooltipProvider delayDuration={300}>
        <RouterProvider router={router} />
      </TooltipProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
