import { createRootRoute, createRoute, createRouter, Outlet } from "@tanstack/react-router";
import App from "../App";

// Root route with layout wrapper
const rootRoute = createRootRoute({
  component: () => <Outlet />,
});

// Index route for the root path - renders App
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: () => <App />,
});

// Build the route tree
const routeTree = rootRoute.addChildren([indexRoute]);

// Create the router instance
export const router = createRouter({
  routeTree,
});

// Register router types
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}