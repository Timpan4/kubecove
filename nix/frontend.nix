{
  lib,
  stdenv,
  autoPatchelfHook,
  bun2nix,
  # "web" builds the browser app kubecove-server embeds.
  runtime ? "native",
}:

let
  root = ../.;
  patchedDependencies = lib.mapAttrs (_: path: root + "/${path}") (
    (lib.importJSON ../package.json).patchedDependencies or { }
  );
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
      ../scripts/dev-interactions-plugin.ts
      ../scripts/frontend-bundle-report.ts
      ../scripts/tsrs.ts
      ../tools/patches
      ../src
      ../svelte.config.js
      ../tsconfig.json
      ../vite.config.ts
    ];
  };

  bunDeps = bun2nix.fetchBunDeps {
    bunNix = ../bun.nix;
    overrides = bun2nix.patchedDependenciesToOverrides { inherit patchedDependencies; };
  };

  nativeBuildInputs = [ autoPatchelfHook ];
  buildInputs = [ stdenv.cc.cc.lib ];

  bunInstallFlags = [
    "--backend=copyfile"
    "--linker=isolated"
  ];

  dontRunLifecycleScripts = true;

  VITE_KUBECOVE_RELEASE_CHANNEL = "dev";
  KUBECOVE_PUBLIC_RUNTIME = runtime;

  # The npm binary needs Nix's linker before the build's TypeScript check runs.
  preBuild = ''
    while IFS= read -r -d "" binary; do
      chmod u+w "$binary"
      autoPatchelf "$binary"
    done < <(find node_modules -type f -path '*/@tsrs/*/tsrs' -print0)
  '';

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
