import {
	ERROR_STATE_CLASS,
	LOADING_SPINNER_CLASS,
	LOADING_STATE_CLASS,
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
				<div className={LOADING_STATE_CLASS}>
					<div className={LOADING_SPINNER_CLASS}></div>
					<span>Loading YAML...</span>
				</div>
			)}
			{yamlError && (
				<div className={ERROR_STATE_CLASS}>
					<p>Error loading YAML: {getErrorMessage(yamlErr)}</p>
				</div>
			)}
			{!yamlLoading && !yamlError && yaml && (
				<pre className={YAML_BLOCK_CLASS}>{yaml}</pre>
			)}
		</>
	);
}
