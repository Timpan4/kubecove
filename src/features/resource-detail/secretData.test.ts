import {
	decodeSecretDataValue,
	isMaskedSecretValue,
	parseSecretData,
} from "./secretData";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

describe("secretData", () => {
	test("parses data and stringData in YAML order", () => {
		expect(
			parseSecretData("data:\n  token: c2VjcmV0\nstringData:\n  note: plain text\n"),
		).toEqual([
			{ id: "data:token", key: "token", value: "c2VjcmV0", source: "data", masked: false },
			{ id: "stringData:note", key: "note", value: "plain text", source: "stringData", masked: false },
		]);
	});

	test("recognizes redacted and provider-masked values", () => {
		expect(isMaskedSecretValue("[REDACTED]")).toBe(true);
		expect(isMaskedSecretValue("REDACTED")).toBe(true);
		expect(isMaskedSecretValue("******")).toBe(true);
		expect(isMaskedSecretValue("c2VjcmV0")).toBe(false);
	});

	test("rejects invalid base64 and decodes valid UTF-8", () => {
		expect(decodeSecretDataValue("not base64")).toEqual({ error: "invalid-base64" });
		expect(decodeSecretDataValue("aMOkbHNh")).toEqual({ value: "hälsa" });
	});

	test("rejects binary bytes", () => {
		expect(decodeSecretDataValue("/w==")).toEqual({ error: "binary-data" });
	});
});
