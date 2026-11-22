import { config as chromeConfig } from "./wdio.real.conf";

export const config = { ...chromeConfig, specs: ["./specs/profile/footprint.e2e.ts"] };
