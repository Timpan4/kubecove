import { describe, expect, test } from "bun:test";
import {
	argoOperationAvailability,
	argoOperationBlocker,
} from "../src/features/gitops/argo-operation-presentation";

describe("Argo operation presentation", () => {
	test("distinguishes available operation review from provider blockers", () => {
		expect(argoOperationAvailability({
			sourceReady: true,
			connectionReady: true,
			transport: "kubernetes",
			connected: false,
		})).toEqual({
			available: true,
			blocker: null,
			reason: "Available. Kubernetes permission is checked during operation review.",
		});
		expect(argoOperationAvailability({
			sourceReady: true,
			connectionReady: false,
			transport: "connected",
			connected: false,
			unavailableReason: "Profile needs credentials.",
		})).toEqual({
			available: false,
			blocker: "provider connection",
			reason: "Profile needs credentials.",
		});
	});

	test("classifies permission and provider failures without inferring authorization", () => {
		expect(argoOperationBlocker("applications.argoproj.io is forbidden")).toBe("permission");
		expect(argoOperationBlocker("provider connection unavailable")).toBe("provider connection");
		expect(argoOperationBlocker("Operation unavailable")).toBe("operation support");
	});
});
