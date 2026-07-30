# Helm reconciliation

The Helm screen inspects releases and compares their decoded manifest references with live Kubernetes objects. It does not run Helm commands or mutate releases.

## Inspect a release

1. Open **Helm** in the intended workspace and context.
2. Use namespace filters and **Search releases...** to find the release.
3. Choose **Cards** or **List**, then select **Details**.
4. Review release namespace, status, revision, chart, and available metadata.
5. Select **Rendered YAML** when you need the manifest stored with the release.
6. Open **Reconciliation** and review every result category.
7. Select **View resources** to inspect matching live objects in the Resource browser.

The overview can summarize **Releases**, **Namespaces**, **Deployed**, **Pending**, and **Failed** states reported by Helm storage. A release status is not proof that every referenced live object matches its intended state.

## Understand reconciliation results

| Result | Meaning |
| --- | --- |
| **Tracked** | A manifest reference and matching live object were found. |
| **Unlabeled live** | The expected live object exists without the explicit release label used by the bounded label scan. |
| **Missing** | A decoded manifest reference has no matching live object in the available result. |
| **Label-only** | A live object has the explicit release label but was not found in the decoded manifest references. |
| **Unavailable** | KubeCove could not establish the comparison result from available evidence. |

KubeCove also performs a bounded scan for explicit `helm.sh/release=<release>` labels. That conservative scan is not proof that Helm owns every labeled object, and it is not a complete scan of every Custom Resource Definition in a cluster.

## Inspection-only boundary

KubeCove does not provide Helm install, upgrade, uninstall, rollback, diff, values-diff, sync, shell execution, or manifest apply from this screen. Raw Helm storage payloads, kubeconfig contents, tokens, and certificates do not cross into the frontend.

When a change is needed, use the release's established values and deployment workflow. Direct Kubernetes changes can be replaced by the next Helm reconciliation and do not update stored release intent.

## Resolve empty or incomplete results

- **No Helm releases**: confirm the context, namespace scope, Helm storage access, and that releases exist there.
- **No matching releases**: clear the search or choose another namespace.
- **No manifest resources decoded**: release storage did not yield usable manifest references.
- **Manifest list truncated by backend.**: comparison covers only the returned bounded list.
- **Loading reconciliation...**: wait before treating counts as final.
- **Helm reconciliation unavailable**: inspect the displayed API or access error and retry after correcting it.
- **No manifest or explicit Helm-labeled live resources were found.**: no evidence was found by either supported method; this is not a universal ownership verdict.

Related: [Resource browser](https://github.com/Timpan4/kubecove/wiki/Resource-Browser), [Guarded Operations](https://github.com/Timpan4/kubecove/wiki/Guarded-Operations), and [GitOps](https://github.com/Timpan4/kubecove/wiki/GitOps-Argo-CD-and-Flux).
