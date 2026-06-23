<script lang="ts">
	import { Pencil, WandSparkles, X } from "lucide-svelte";
	import YamlCodeEditor from "@/components/YamlCodeEditor.svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		ScrollArea,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		Spinner,
		TabsContent,
	} from "@/components/ui/svelte";
	import { getErrorMessage } from "./helpers";
	import { diffLineClassName } from "./yamlTabDiff";

	let {
		yamlQuery,
		yamlText,
		yamlApplyTarget,
		yamlAppliedMessage,
		yamlEditing,
		yamlViewMode,
		yamlEncoding,
		yamlLoadingDraft,
		yamlPreparing,
		yamlApplying,
		yamlDraft = $bindable(""),
		yamlApplyDisabledReason,
		yamlLintError,
		yamlLintNotes,
		yamlFormatError,
		yamlPrepareError,
		yamlApplyError,
		canAllowYamlForceConflicts,
		yamlPreview,
		yamlShowFullDiff = $bindable(false),
		visibleYamlDiffLines,
		hiddenYamlDiffCount,
		yamlLintDiagnostics,
		yamlErrorLensEnabled,
		setYamlViewMode,
		setYamlEncoding,
		resetYamlApply,
		startYamlApplyEdit,
		formatYamlDraft,
		previewYamlApply,
		applyYamlPreview,
		allowYamlForceConflictsForResource,
		kubernetesYamlDiagnostics,
		clearYamlDraftFeedback,
	} = $props();
</script>

<TabsContent value="yaml" class="min-h-0">
		{#if yamlQuery.isPending && !yamlText}
			<div class="flex min-h-32 items-center justify-center gap-2 text-muted-foreground">
				<Spinner />
				<span>Loading YAML</span>
			</div>
		{:else if yamlQuery.isError}
			<Alert variant="destructive">
				<AlertTitle>Failed to load YAML</AlertTitle>
				<AlertDescription>{getErrorMessage(yamlQuery.error)}</AlertDescription>
			</Alert>
		{:else}
			<div class="flex flex-col gap-3">
				<div class="flex flex-col gap-2">
					<div class="flex flex-wrap items-center justify-between gap-2">
						<div class="flex min-w-0 flex-wrap items-center gap-2">
							<Badge variant="outline" class="max-w-full truncate">{yamlApplyTarget}</Badge>
							{#if yamlAppliedMessage}
								<Badge variant="secondary">{yamlAppliedMessage}</Badge>
							{/if}
						</div>
						<div class="flex flex-wrap items-center gap-2">
							<div class="flex items-center gap-1.5">
								<span class="text-[11px] font-medium text-muted-foreground">YAML shape</span>
								<Select
									value={yamlEditing ? "applyClean" : yamlViewMode}
									onValueChange={setYamlViewMode}
									disabled={yamlEditing}
								>
									<SelectTrigger class="h-8 w-36" aria-label="YAML shape">
										<SelectValue>{yamlEditing || yamlViewMode === "applyClean" ? "Apply clean" : "Kubectl view"}</SelectValue>
									</SelectTrigger>
									<SelectContent>
										<SelectGroup>
											<SelectItem value="kubectl" label="Kubectl view">Kubectl view</SelectItem>
											<SelectItem value="applyClean" label="Apply clean">Apply clean</SelectItem>
										</SelectGroup>
									</SelectContent>
								</Select>
							</div>
							<div class="flex items-center gap-1.5">
								<span class="text-[11px] font-medium text-muted-foreground">Encoding</span>
								<Select
									value={yamlEncoding}
									onValueChange={setYamlEncoding}
									disabled={yamlEditing}
								>
									<SelectTrigger class="h-8 w-28" aria-label="YAML encoding">
										<SelectValue>{yamlEncoding === "kyaml" ? "KYAML" : "YAML"}</SelectValue>
									</SelectTrigger>
									<SelectContent>
										<SelectGroup>
											<SelectItem value="yaml" label="YAML">YAML</SelectItem>
											<SelectItem value="kyaml" label="KYAML">KYAML</SelectItem>
										</SelectGroup>
									</SelectContent>
								</Select>
							</div>
							{#if yamlEditing}
								<Button type="button" variant="outline" onclick={resetYamlApply}>
									<X data-icon="inline-start" />
									Cancel
								</Button>
							{:else}
								<Button
									type="button"
									variant="secondary"
									disabled={Boolean(yamlApplyDisabledReason) || yamlLoadingDraft}
									onclick={startYamlApplyEdit}
								>
									{#if yamlLoadingDraft}<Spinner data-icon="inline-start" />{/if}
									<Pencil data-icon="inline-start" />
									Edit YAML
								</Button>
							{/if}
						</div>
					</div>
					{#if yamlEditing}
						<div class="flex flex-wrap items-center justify-end gap-2">
							<Button
								type="button"
								variant="outline"
								disabled={yamlLoadingDraft || yamlPreparing || yamlApplying}
								onclick={formatYamlDraft}
							>
								<WandSparkles data-icon="inline-start" />
								Format ({yamlEncoding.toUpperCase()})
							</Button>
							<Button
								type="button"
								variant="secondary"
								disabled={yamlPreparing || yamlDraft.trim().length === 0}
								onclick={() => void previewYamlApply()}
							>
								{#if yamlPreparing}<Spinner data-icon="inline-start" />{/if}
								Dry run
							</Button>
							<span
								class="group relative inline-flex"
								title={!yamlPreview ? "Run a dry run before applying." : undefined}
							>
								<Button
									type="button"
									disabled={!yamlPreview || yamlApplying}
									aria-describedby={!yamlPreview ? "yaml-apply-disabled-tooltip" : undefined}
									onclick={applyYamlPreview}
								>
									{#if yamlApplying}<Spinner data-icon="inline-start" />{/if}
									Apply
								</Button>
								{#if !yamlPreview}
									<span
										id="yaml-apply-disabled-tooltip"
										role="tooltip"
										class="pointer-events-none absolute right-0 top-full z-50 mt-1 hidden w-max max-w-xs rounded-md bg-foreground px-2 py-1 text-xs text-background shadow group-hover:block"
									>
										Run a dry run before applying.
									</span>
								{/if}
							</span>
						</div>
					{/if}
				</div>

				{#if yamlApplyDisabledReason}
					<Alert>
						<AlertTitle>YAML apply unavailable</AlertTitle>
						<AlertDescription>{yamlApplyDisabledReason}</AlertDescription>
					</Alert>
				{/if}
				{#if yamlLintError}
					<Alert variant="destructive">
						<AlertTitle>YAML lint failed</AlertTitle>
						<AlertDescription>{yamlLintError}</AlertDescription>
					</Alert>
				{:else if yamlLintNotes.length > 0}
					<Alert>
						<AlertTitle>YAML lint status</AlertTitle>
						<AlertDescription>
							<div class="flex flex-col gap-1">
								{#each yamlLintNotes as note (`${note.severity}:${note.source}:${note.message}`)}
									<span>{note.source}: {note.message}</span>
								{/each}
							</div>
						</AlertDescription>
					</Alert>
				{/if}
				{#if yamlFormatError}
					<Alert variant="destructive">
						<AlertTitle>Format failed</AlertTitle>
						<AlertDescription>{yamlFormatError}</AlertDescription>
					</Alert>
				{/if}
				{#if yamlPrepareError}
					<Alert variant="destructive">
						<AlertTitle>Dry run failed</AlertTitle>
						<AlertDescription>
							<div class="flex flex-col gap-3">
								<span>{yamlPrepareError}</span>
								{#if canAllowYamlForceConflicts}
									<div>
										<Button
											type="button"
											variant="destructive"
											size="sm"
											disabled={yamlPreparing || yamlApplying}
											onclick={allowYamlForceConflictsForResource}
										>
											Allow force-conflicts for this resource
										</Button>
									</div>
								{/if}
							</div>
						</AlertDescription>
					</Alert>
				{/if}
				{#if yamlApplyError}
					<Alert variant="destructive">
						<AlertTitle>YAML apply failed</AlertTitle>
						<AlertDescription>{yamlApplyError}</AlertDescription>
					</Alert>
				{/if}

				{#if yamlEditing}
					{#if yamlPreview}
						<div class="rounded-md border bg-background/50">
							<div class="flex items-center justify-between gap-2 border-b px-3 py-2">
								<div class="text-xs font-medium">
									Dry-run diff ({yamlShowFullDiff ? "full" : "compact"})
								</div>
								<Button
									type="button"
									variant="ghost"
									size="sm"
									onclick={() => (yamlShowFullDiff = !yamlShowFullDiff)}
								>
									{yamlShowFullDiff ? "Collapse diff" : "Show full diff"}
								</Button>
							</div>
							<ScrollArea class="max-h-72">
								<div class="overflow-x-auto py-1 font-mono text-xs leading-relaxed">
									{#each visibleYamlDiffLines as line, index (`${index}:${line.text}`)}
										<div class={`whitespace-pre px-3 ${diffLineClassName(line.type)}`}>{line.text || " "}</div>
									{/each}
								</div>
							</ScrollArea>
							{#if hiddenYamlDiffCount > 0}
								<div class="border-t px-3 py-2 text-xs text-muted-foreground">
									{hiddenYamlDiffCount} hidden diff lines
								</div>
							{/if}
						</div>
					{/if}
					<YamlCodeEditor
						bind:value={yamlDraft}
						editable
						minHeight="420px"
						extraDiagnostics={kubernetesYamlDiagnostics}
						showErrorLens={yamlErrorLensEnabled}
						onChange={clearYamlDraftFeedback}
					/>
					{#if yamlLintDiagnostics.length > 0}
						<div class="rounded-md border bg-background/50">
							<div class="border-b px-3 py-2 text-xs font-medium">YAML diagnostics</div>
							<div class="flex flex-col divide-y">
								{#each yamlLintDiagnostics as diagnostic (`${diagnostic.severity}:${diagnostic.source}:${diagnostic.message}:${diagnostic.fieldPath ?? ""}`)}
									<div class="grid gap-1 px-3 py-2 text-xs sm:grid-cols-[5rem_minmax(0,1fr)]">
										<Badge variant={diagnostic.severity === "error" ? "destructive" : "outline"}>
											{diagnostic.severity}
										</Badge>
										<div class="min-w-0">
											<div class="break-words font-medium">{diagnostic.message}</div>
											<div class="mt-1 break-words text-muted-foreground">
							{diagnostic.source}{diagnostic.fieldPath ? ` - ${diagnostic.fieldPath}` : ""}
											</div>
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				{:else}
					<YamlCodeEditor value={yamlText} minHeight="520px" />
				{/if}
			</div>
		{/if}
	</TabsContent>

