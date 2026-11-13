import type { Contract } from "./contract";
import { string } from "./contract-values";

/** An absent selection means the full contract; [] explicitly selects nothing. */
export function selectedContract(contract: Contract, json: string | undefined): Contract {
	if (!json) return contract;
	const value: unknown = JSON.parse(json);
	if (!Array.isArray(value)) throw new Error("Parity selection must be an array of scenario IDs");
	const remaining = new Set(contract.scenarios.map(({ id }) => id));
	const ids = new Set<string>();
	for (const item of value) {
		const id = string(item);
		if (!remaining.delete(id)) throw new Error(`Unknown or duplicate selected scenario: ${id}`);
		ids.add(id);
	}
	return { scenarios: contract.scenarios.filter(({ id }) => ids.has(id)) };
}
