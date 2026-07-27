#!/usr/bin/env node

const Module = require("node:module");
const path = require("node:path");

const typescript6Root = path.dirname(require.resolve("typescript-6/package.json"));
const resolveFilename = Module._resolveFilename;

Module._resolveFilename = function resolveWithTypescript6(
	request,
	parent,
	isMain,
	options,
) {
	if (request === "typescript") {
		return resolveFilename.call(this, "typescript-6", parent, isMain, options);
	}

	if (request.startsWith("typescript/")) {
		return resolveFilename.call(
			this,
			path.join(typescript6Root, request.slice("typescript/".length)),
			parent,
			isMain,
			options,
		);
	}

	return resolveFilename.call(this, request, parent, isMain, options);
};

require("svelte-check");
