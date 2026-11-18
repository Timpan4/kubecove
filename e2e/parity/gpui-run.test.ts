import { expect, test } from "bun:test";
import { parityRebuildReason } from "./gpui-run";

const fingerprint = 'INFO prepare_target{target="parity"}: cargo::compiler::fingerprint:';

test("reports the parity target's own dirty fingerprint", () => {
	const dependency = 'INFO prepare_target{target="gpui"}: cargo::compiler::fingerprint: dirty: dependency source changed';
	const parity = `${fingerprint}     dirty: the file tests/parity.rs has changed`;
	expect(parityRebuildReason(`${dependency}\n${parity}`)).toBe("the file tests/parity.rs has changed");
	expect(parityRebuildReason(dependency)).toBeUndefined();
});

test("reports the fingerprint read cause as Cargo logged it", () => {
	const failed = `${fingerprint}     err: failed to read fingerprint/test-integration-test-parity`;
	const unix = "No such file or directory (os error 2)";
	const windows = "The system cannot find the path specified. (os error 3)";
	expect(parityRebuildReason(`${failed}\n\nCaused by:\n    ${unix}`)).toBe(`Cargo could not read the parity fingerprint: ${unix}`);
	expect(parityRebuildReason(`${failed}\r\n\r\nCaused by:\r\n    ${windows}`)).toBe(`Cargo could not read the parity fingerprint: ${windows}`);
	expect(parityRebuildReason(failed)).toBeUndefined();
});
