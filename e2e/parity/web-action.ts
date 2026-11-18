import type { Page } from "playwright-core";
import type { Action, Step } from "./contract";

export type WebActions = {
	[Name in Action]: (page: Page, step: Step<Name>, scenarioSteps: readonly Step[]) => Promise<void>;
};

export function sharedActions<Name extends Action>(names: readonly [Name, ...Name[]], handler: (page: Page, step: Step<Name>, scenarioSteps: readonly Step[]) => Promise<void>): Pick<WebActions, Name> {
	// SAFETY: The handler accepts every step named by the registered keys.
	return Object.fromEntries(names.map((name) => [name, handler])) as Pick<WebActions, Name>;
}
