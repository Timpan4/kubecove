import { connect } from "node:net";

// The test server owns completion, so login delay needs no sleep or deadline.
const socket = connect({ host: "127.0.0.1", port: Number(process.argv[2]) });
socket.on("error", () => process.exit(1));
socket.once("data", () => {
	process.stdout.write(JSON.stringify({
		apiVersion: "client.authentication.k8s.io/v1",
		kind: "ExecCredential",
		status: { token: "native-exec-fixture", expirationTimestamp: "2099-01-01T00:00:00Z" },
	}));
	socket.end();
});
