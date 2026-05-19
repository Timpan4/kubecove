import { ExactTimestampText } from "@/components/TimestampText";
import type { IncidentSignal } from "./helpers";

export function IncidentSignalValue({ signal }: { signal: IncidentSignal }) {
	const parts = signal.valueParts ?? [
		{ kind: "text" as const, text: signal.value },
	];

	return (
		<>
			{parts.map((part, index) =>
				part.kind === "timestamp" ? (
					<ExactTimestampText
						key={`${part.value}:${index}`}
						value={part.value}
						className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
					/>
				) : (
					<span key={`${part.text}:${index}`}>{part.text}</span>
				),
			)}
		</>
	);
}
