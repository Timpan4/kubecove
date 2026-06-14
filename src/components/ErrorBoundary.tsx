import { Component, type ErrorInfo, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { diagnosticLog } from "@/lib/diagnostics";

interface ErrorBoundaryProps {
	label: string;
	children: ReactNode;
}

interface ErrorBoundaryState {
	error: Error | null;
}

export class ErrorBoundary extends Component<
	ErrorBoundaryProps,
	ErrorBoundaryState
> {
	state: ErrorBoundaryState = { error: null };

	static getDerivedStateFromError(error: Error): ErrorBoundaryState {
		return { error };
	}

	componentDidCatch(error: Error, info: ErrorInfo): void {
		diagnosticLog("app.render.error", {
			label: this.props.label,
			error: error.message,
			componentStack: info.componentStack ?? "",
		});
	}

	render(): ReactNode {
		if (!this.state.error) return this.props.children;

		return (
			<div className="flex h-full min-h-0 items-center justify-center p-6">
				<div className="grid max-w-md gap-3 rounded-lg border bg-card p-4 text-card-foreground shadow-sm">
					<div className="grid gap-1">
						<h2 className="text-sm font-semibold">Could not render {this.props.label}</h2>
						<p className="text-xs/relaxed text-muted-foreground">
							{this.state.error.message}
						</p>
					</div>
					<Button
						type="button"
						variant="outline"
						className="w-fit"
						onClick={() => this.setState({ error: null })}
					>
						Try again
					</Button>
				</div>
			</div>
		);
	}
}
