import { config as nativeConfig } from "./wdio.real.conf";

export const config = { ...nativeConfig, specs: ["./specs/profile/footprint.e2e.ts"] };
