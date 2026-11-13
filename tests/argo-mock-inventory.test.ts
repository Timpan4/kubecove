import { expect, test } from "bun:test";
import { argoApplicationSummary, argoMockHandlers, configureArgoInventory } from "../src/lib/tauri-dev-mock-argo";

test("every generated inventory Application retains its identity and health in detail reads", () => {
	configureArgoInventory("many");
	try {
		const handlers = argoMockHandlers(() => "");
		for (const app of handlers.list_argocd_applications().items) {
			const details = handlers.get_argocd_application_details({ name: app.name, namespace: app.namespace });
			const inspector = handlers.get_argo_application_inspector({ application: { name: app.name, namespace: app.namespace, context: app.cluster } });
			expect(details.summary).toEqual(app);
			expect(inspector.application.uid).toBe(`mock-${app.name}`);
			expect(inspector.status.health.status).toBe(app.healthStatus);
			expect(argoApplicationSummary(app.name)?.name).toBe(app.name);
			expect(argoApplicationSummary(app.name)?.healthAssessment).toEqual(app.healthAssessment);
		}
	} finally {
		configureArgoInventory("base");
	}
});
