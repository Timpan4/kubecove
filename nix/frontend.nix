{
  lib,
  bun2nix,
}:

let
  root = ../.;
in
bun2nix.mkDerivation {
  packageJson = ../package.json;

  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      ../bun.lock
      ../index.html
      ../package.json
      ../public
      ../src
      ../svelte.config.js
      ../tsconfig.json
      ../tsconfig.node.json
      ../vite.config.ts
    ];
  };

  bunDeps = bun2nix.fetchBunDeps {
    bunNix = ../bun.nix;
  };

  bunInstallFlags = [
    "--backend=copyfile"
    "--linker=isolated"
  ];

  dontRunLifecycleScripts = true;

  VITE_KUBECOVE_RELEASE_CHANNEL = "dev";

  buildPhase = ''
    runHook preBuild
    bun run build
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    cp -r dist "$out"
    runHook postInstall
  '';
}
