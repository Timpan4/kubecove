import type { Page } from "playwright-core";
import type { Action, Step } from "./contract";

export type WebActions = {
	[Name in Action]: (page: Page, step: Step<Name>, scenarioSteps: readonly Step[]) => Promise<void>;
};

export function sharedActions<Name extends Action>(names: readonly Name[], handler: (page: Page, step: Step<Name>, scenarioSteps: readonly Step[]) => Promise<void>): Pick<WebActions, Name> {
	// The handler accepts every step named by the keys in this registration.
	return Object.fromEntries(names.map((name) => [name, handler])) as Pick<WebActions, Name>;
}
