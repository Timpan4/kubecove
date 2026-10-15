# GitOps

Native Argo CD, Flux and Helm inspection, connected Argo transport/credentials, reviewed Argo operations and private Service tunnels. These remain backend-only domain implementations under the existing ADRs.

Depends on models, Kubernetes infrastructure and live forwarding; it does not depend on the inspection command crate. Helm uses shared dynamic projections from Kubernetes infrastructure. Cross-domain workspace cleanup stays in the host composition root, avoiding a reverse dependency from cancellation infrastructure to Argo connections.
