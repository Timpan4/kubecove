import { Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { YamlEncoding, YamlViewMode } from "@/lib/types";

interface YamlEncodingControlProps {
	value: YamlEncoding;
	onChange: (encoding: YamlEncoding) => void;
	disabled?: boolean;
}

interface YamlViewModeControlProps {
	value: YamlViewMode;
	onChange: (mode: YamlViewMode) => void;
	disabled?: boolean;
}

export function YamlEncodingControl({
	value,
	onChange,
	disabled = false,
}: YamlEncodingControlProps) {
	return (
		<div className="flex rounded-md border bg-background/50 p-0.5">
			<YamlToggleButton
				value="yaml"
				label="YAML"
				selected={value === "yaml"}
				onSelect={onChange}
				disabled={disabled}
			/>
			<YamlToggleButton
				value="kyaml"
				label="KYAML"
				selected={value === "kyaml"}
				onSelect={onChange}
				disabled={disabled}
			/>
		</div>
	);
}

export function YamlViewModeControl({
	value,
	onChange,
	disabled = false,
}: YamlViewModeControlProps) {
	return (
		<div className="flex rounded-md border bg-background/50 p-0.5">
			<YamlToggleButton
				value="kubectl"
				label="Kubectl"
				selected={value === "kubectl"}
				onSelect={onChange}
				disabled={disabled}
			/>
			<YamlToggleButton
				value="applyClean"
				label="Apply-friendly"
				selected={value === "applyClean"}
				onSelect={onChange}
				disabled={disabled}
			/>
		</div>
	);
}

function YamlToggleButton<TValue extends string>({
	value,
	label,
	selected,
	onSelect,
	disabled,
}: {
	value: TValue;
	label: string;
	selected: boolean;
	onSelect: (value: TValue) => void;
	disabled: boolean;
}) {
	return (
		<Button
			type="button"
			variant={selected ? "secondary" : "ghost"}
			size="sm"
			className="h-8 rounded-sm px-2.5"
			onClick={() => onSelect(value)}
			disabled={disabled}
			aria-pressed={selected}
		>
			{selected && <Check data-icon="inline-start" />}
			{label}
		</Button>
	);
}
