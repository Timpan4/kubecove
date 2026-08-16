{
  pkgs ? null,
  bun2nix ? null,
  system ? builtins.currentSystem,
}:

if pkgs == null || bun2nix == null then
  let
    lock = builtins.fromJSON (builtins.readFile ./flake.lock);
    flakeCompat = lock.nodes.flake-compat.locked;
    flakeCompatSrc = builtins.fetchTarball {
      url = "https://github.com/${flakeCompat.owner}/${flakeCompat.repo}/archive/${flakeCompat.rev}.tar.gz";
      sha256 = flakeCompat.narHash;
    };
    source = builtins.path {
      path = ./.;
      name = "kubecove-source";
      filter =
        path: _type:
        let
          name = builtins.baseNameOf path;
        in
        !builtins.elem name [
          ".git"
          "dist"
          "node_modules"
          "result"
          "target"
        ]
        && builtins.match "\\.nix-.*\\.log" name == null;
    };
    compat = import flakeCompatSrc {
      src = source;
      inherit system;
    };
  in
  compat.outputs.packages.${system}
else
  let
    frontend = pkgs.callPackage ./nix/frontend.nix {
      inherit bun2nix;
    };
    kubecove = pkgs.callPackage ./nix/kubecove.nix {
      inherit frontend;
    };
  in
  {
    inherit frontend kubecove;
    default = kubecove;
  }
