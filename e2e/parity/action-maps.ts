type AreaKey<Area> = Area extends object ? keyof Area & string : never;
type AreaValue<Area, Key extends string> = Area extends Record<Key, infer Value> ? Value : never;
type ActionHandler = (...args: never[]) => void;

type ActionMap<Area> = {
	[Key in AreaKey<Area>]: AreaValue<Area, Key>;
};

/** Combines area registrations and rejects a key claimed by two areas. */
export function mergeActionMaps<const Areas extends readonly Record<string, ActionHandler>[]>(...areas: Areas): ActionMap<Areas[number]> {
	const entries = new Map<string, unknown>();
	for (const area of areas) {
		const prototype = Object.getPrototypeOf(area);
		if (prototype !== Object.prototype && prototype !== null) throw new Error("Expected a plain parity action map");
		for (const action of Object.getOwnPropertyNames(area)) {
			if (entries.has(action)) throw new Error(`Duplicate parity action: ${action}`);
			entries.set(action, area[action]);
		}
	}
	// SAFETY: The entries contain each own action key and its value from the input maps.
	return Object.fromEntries(entries) as ActionMap<Areas[number]>;
}
