import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Spinner } from "@/components/ui/spinner";
import {
	YAML_BLOCK_CLASS,
} from "./constants";
import { getErrorMessage } from "./helpers";

interface YamlTabProps {
	yaml: string | undefined;
	yamlLoading: boolean;
	yamlError: boolean;
	yamlErr: unknown;
}

export function YamlTab({
	yaml,
	yamlLoading,
	yamlError,
	yamlErr,
}: YamlTabProps) {
	return (
		<>
			{yamlLoading && (
				<div className="p-6 text-center text-xs text-muted-foreground">
					<Spinner className="mx-auto mb-2 size-4" />
					<span>Loading YAML...</span>
				</div>
			)}
			{yamlError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to load YAML</AlertTitle>
					<AlertDescription>{getErrorMessage(yamlErr)}</AlertDescription>
				</Alert>
			)}
			{!yamlLoading && !yamlError && yaml && (
				<pre className={YAML_BLOCK_CLASS}>{yaml}</pre>
			)}
		</>
	);
}
