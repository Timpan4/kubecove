{
  lib,
  stdenv,
  rustPlatform,
  cargo-tauri,
  glib-networking,
  gtk3,
  kubeconform,
  libayatana-appindicator,
  libsecret,
  librsvg,
  openssl,
  pkg-config,
  webkitgtk_4_1,
  wrapGAppsHook3,
  frontend,
}:

let
  version = (builtins.fromTOML (builtins.readFile ../src-tauri/Cargo.toml)).package.version;
  tauriConfig = builtins.toJSON {
    build = {
      beforeBuildCommand = "";
      frontendDist = "${frontend}";
    };
    bundle = {
      createUpdaterArtifacts = false;
      externalBin = [ ];
    };
  };
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "kubecove";
  inherit version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = ../src-tauri;
  };

  cargoHash = "sha256-TgfzBNFWgF0I///A/qlePAPGhuXBeJ5g4jf77F02OVc=";
  doCheck = false;

  nativeBuildInputs = [
    cargo-tauri.hook
    pkg-config
  ]
  ++ lib.optionals stdenv.hostPlatform.isLinux [ wrapGAppsHook3 ];

  buildInputs = lib.optionals stdenv.hostPlatform.isLinux [
    glib-networking
    gtk3
    libayatana-appindicator
    libsecret
    librsvg
    openssl
    webkitgtk_4_1
  ];

  cargoRoot = "src-tauri";
  buildAndTestSubdir = finalAttrs.cargoRoot;
  tauriBuildFlags = [
    "--config"
    tauriConfig
  ];

  preFixup = lib.optionalString stdenv.hostPlatform.isLinux ''
    gappsWrapperArgs+=(--set KUBECOVE_KUBECONFORM "${lib.getExe kubeconform}")
  '';

  meta = {
    description = "A context-first desktop workspace for Kubernetes operations";
    homepage = "https://github.com/Timpan4/kubecove";
    license = lib.licenses.agpl3Plus;
    mainProgram = "kubecove";
    platforms = lib.platforms.linux;
  };
})
