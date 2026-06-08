import { readWorkspaceReleaseVersions } from "./release-versions";
import { readChangelogReleaseBody } from "./release-notes";

const version = Bun.argv[2] ?? readWorkspaceReleaseVersions().packageVersion;
console.log(readChangelogReleaseBody(version));
