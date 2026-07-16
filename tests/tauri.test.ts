import { describe, test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import type { ResourceSummary } from "../src/lib/types";
import {
	conditionStatusTone,
	getContainerStatusRows,
	getConditionRows,
	resourceReadyLabel,
	resourceReadyTone,
	shouldFetchResourceDetails,
} from "../src/features/resource-detail/helpers";
import {
	buildCuratedMetadata,
	visibleMetadataBadges,
} from "../src/features/resource-detail/metadata-details";

describe("shouldFetchResourceDetails", () => {
  const resource = {
    cluster: "minikube",
    kind: "Pod",
    name: "test-pod",
    namespace: "default",
    age: "1m",
  };

  test("fetches full details when a valid resource is selected", () => {
    expect(shouldFetchResourceDetails(resource)).toBe(true);
  });

  test("does not fetch details until a complete resource identity exists", () => {
    expect(shouldFetchResourceDetails({ ...resource, name: "" })).toBe(false);
  });
});

describe("buildCuratedMetadata", () => {
	const resource: ResourceSummary = {
		cluster: "kind-prod",
		kind: "Pod",
		name: "todo-web-97bfcd566-jx9mb",
		namespace: "todo",
		age: "11d",
		apiVersion: "v1",
		health: "healthy",
		ownerRef: "ReplicaSet todo-web-97bfcd566",
		argoApp: "todo",
	};

	test("extracts identity and lifecycle fields from Kubernetes metadata", () => {
		const metadata = buildCuratedMetadata(
			{
				name: "todo-web-97bfcd566-jx9mb",
				namespace: "todo",
				uid: "7335932b-8a03-42ac-b1d9-838bc1da55e0",
				resourceVersion: "192041",
				creationTimestamp: "2026-06-10T16:50:37Z",
				generation: 1,
				finalizers: [],
				generateName: "todo-web-97bfcd566-",
			},
			resource,
		);

		expect(metadata.identity).toEqual([
			{ label: "Name", value: "todo-web-97bfcd566-jx9mb" },
			{ label: "Namespace", value: "todo" },
			{ label: "UID", value: "7335932b-8a03-42ac-b1d9-838bc1da55e0" },
			{ label: "Resource Version", value: "192041" },
		]);
		expect(metadata.lifecycle).toContainEqual({
			label: "Deletion",
			value: "not scheduled",
		});
		expect(metadata.lifecycle).toContainEqual({ label: "Finalizers", value: "none" });
		expect(metadata.naming).toContainEqual({
			label: "Generate name",
			value: "todo-web-97bfcd566-",
		});
	});

	test("chooses controller owner reference before other owners", () => {
		const metadata = buildCuratedMetadata(
			{
				ownerReferences: [
					{ kind: "Job", name: "backup", uid: "job-uid" },
					{
						kind: "ReplicaSet",
						name: "todo-web-97bfcd566",
						uid: "rs-uid",
						controller: true,
					},
				],
			},
			resource,
		);

		expect(metadata.ownership).toContainEqual({
			label: "Controller",
			value: "ReplicaSet",
		});
		expect(metadata.ownership).toContainEqual({
			label: "Owner",
			value: "todo-web-97bfcd566",
		});
		expect(metadata.ownership).toContainEqual({ label: "Owner UID", value: "rs-uid" });
	});

	test("sorts labels and exposes truncation counts", () => {
		const metadata = buildCuratedMetadata(
			{
				labels: {
					"pod-template-hash": "97bfcd566",
					"app.kubernetes.io/name": "todo-web",
					"gitops.toolkit": "argocd",
					"app.kubernetes.io/part-of": "kubecove-lab",
					"tier": "frontend",
				},
			},
			resource,
		);
		const visible = visibleMetadataBadges(metadata.labels, false);

		expect(metadata.labels.map((label) => label.key)).toEqual([
			"app.kubernetes.io/name",
			"app.kubernetes.io/part-of",
			"gitops.toolkit",
			"pod-template-hash",
			"tier",
		]);
		expect(visible.badges).toHaveLength(4);
		expect(visible.hiddenCount).toBe(1);
		expect(visibleMetadataBadges(metadata.labels, true).hiddenCount).toBe(0);
	});

	test("summarizes short annotations and keeps noisy values out of default view", () => {
		const metadata = buildCuratedMetadata(
			{
				annotations: {
					"checksum/config": "123456",
					"kubectl.kubernetes.io/last-applied-configuration": "{\"kind\":\"Pod\"}",
					"example.com/notes": "line one\nline two",
					"example.com/long": "x".repeat(120),
				},
			},
			resource,
		);

		expect(metadata.annotationCount).toBe(4);
		expect(metadata.annotations).toEqual([
			{ key: "checksum/config", value: "123456" },
		]);
	});

	test("summarizes managedFields managers without exposing raw field maps", () => {
		const metadata = buildCuratedMetadata(
			{
				managedFields: [
					{ manager: "kubelet", fieldsV1: { "f:status": {} } },
					{ manager: "kube-controller-manager", fieldsV1: { "f:metadata": {} } },
					{ manager: "kubelet", fieldsV1: { "f:spec": {} } },
				],
			},
			resource,
		);

		expect(metadata.management).toEqual([
			{ label: "Managers", value: "kubelet, kube-controller-manager" },
			{ label: "Raw managedFields", value: "advanced metadata" },
		]);
	});
});

describe("getConditionRows", () => {
  test("extracts readable Kubernetes condition rows from status", () => {
    expect(
      getConditionRows({
        conditions: [
          {
            type: "Ready",
            status: "False",
            reason: "ContainersNotReady",
            message: "containers with unready status",
            lastTransitionTime: "2026-05-17T11:55:00Z",
          },
        ],
      }),
    ).toEqual([
      {
        type: "Ready",
        status: "False",
        reason: "ContainersNotReady",
        message: "containers with unready status",
        lastTransitionTime: "2026-05-17T11:55:00Z",
      },
    ]);
  });

  test("returns an empty list when status conditions are absent", () => {
    expect(getConditionRows({ phase: "Running" })).toEqual([]);
  });
});

describe("resource detail status tones", () => {
	const completedPod: ResourceSummary = {
		cluster: "kind-prod",
		kind: "Pod",
		name: "job-pod",
		namespace: "jobs-lab",
		age: "6m",
		health: "healthy",
		status: "Succeeded",
		ready: "False",
	};

	test("shows completed pods as not-dangerous when readiness is false", () => {
		expect(resourceReadyLabel(completedPod)).toBe("Completed");
		expect(resourceReadyTone(completedPod)).toBe("success");
	});

	test("renders complete phase chips as success", () => {
		const source = readFileSync(
			"src/features/resource-detail/resource-status.ts",
			"utf8",
		);

		expect(source).toContain('"complete"');
		expect(source).toContain('"completed"');
		expect(source).toContain('"Completed"');
	});

	test("uses neutral condition chips for expected completed pod false conditions", () => {
		expect(
			conditionStatusTone(
				{ type: "Ready", status: "False", reason: "PodCompleted" },
				completedPod,
			),
		).toBe("neutral");
		expect(
			conditionStatusTone(
				{ type: "ContainersReady", status: "False", reason: "PodCompleted" },
				completedPod,
			),
		).toBe("neutral");
		expect(
			conditionStatusTone(
				{ type: "PodReadyToStartContainers", status: "False" },
				completedPod,
			),
		).toBe("neutral");
	});

	test("keeps active not-ready pods red", () => {
		const runningPod = {
			...completedPod,
			status: "Running",
			ready: "False",
			health: "degraded" as const,
		};

		expect(resourceReadyLabel(runningPod)).toBe("Not ready");
		expect(resourceReadyTone(runningPod)).toBe("error");
		expect(
			conditionStatusTone(
				{ type: "Ready", status: "False", reason: "ContainersNotReady" },
				runningPod,
			),
		).toBe("error");
	});

	test("marks true failure conditions as warning", () => {
		const failedJob = {
			...completedPod,
			kind: "Job",
			status: "Failed",
			ready: undefined,
			health: "degraded" as const,
		};

		expect(
			conditionStatusTone(
				{
					type: "FailureTarget",
					status: "True",
					reason: "BackoffLimitExceeded",
				},
				failedJob,
			),
		).toBe("warning");
		expect(
			conditionStatusTone(
				{ type: "Failed", status: "True", reason: "BackoffLimitExceeded" },
				failedJob,
			),
		).toBe("warning");
	});
});

describe("getContainerStatusRows", () => {
	test("extracts container restart context with timestamps", () => {
		expect(
			getContainerStatusRows({
				containerStatuses: [
					{
						name: "api",
						ready: true,
						restartCount: 1,
						state: { running: { startedAt: "2026-05-17T11:56:00Z" } },
						lastState: {
							terminated: {
								reason: "Error",
								exitCode: 1,
								startedAt: "2026-05-17T11:45:00Z",
								finishedAt: "2026-05-17T11:55:00Z",
							},
						},
					},
				],
			}),
		).toEqual([
			{
				name: "api",
				type: "container",
				ready: true,
				restartCount: 1,
				state: "running",
				startedAt: "2026-05-17T11:56:00Z",
				lastState: "terminated",
				lastReason: "Error",
				lastExitCode: 1,
				lastStartedAt: "2026-05-17T11:45:00Z",
				lastFinishedAt: "2026-05-17T11:55:00Z",
			},
		]);
	});
});

describe("sidebar source safeguards", () => {
	test("uses query state for namespace loading", () => {
		const source = readFileSync("src/features/resources/NamespaceList.svelte", "utf8");

		expect(source).toContain("createQuery");
		expect(source).toContain("queryKeys.namespaces");
		expect(source).not.toContain("requestSeq");
		expect(source).not.toContain("$effect");
	});

	test("uses non-submit button types for sidebar controls", () => {
		const buttonSource = readFileSync("src/components/ui/svelte/Button.svelte", "utf8");
		const menuSource = readFileSync(
			"src/components/ui/svelte/SidebarMenuButton.svelte",
			"utf8",
		);

		expect(buttonSource).toContain('type = "button"');
		expect(buttonSource).toContain("{type}");
		expect(menuSource).toContain('type = "button"');
		expect(menuSource).toContain("{type}");
	});
});
