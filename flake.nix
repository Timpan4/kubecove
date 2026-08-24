{
  description = "KubeCove";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default-linux";

    bun2nix = {
      url = "github:nix-community/bun2nix/2.1.2";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.systems.follows = "systems";
    };

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      systems,
      bun2nix,
      ...
    }:
    let
      eachSystem = nixpkgs.lib.genAttrs (import systems);
      packagesFor =
        system:
        import ./default.nix {
          pkgs = import nixpkgs { inherit system; };
          bun2nix = bun2nix.packages.${system}.default;
          inherit system;
        };
    in
    {
      packages = eachSystem packagesFor;

      apps = eachSystem (
        system:
        let
          packages = packagesFor system;
          app = {
            type = "app";
            program = "${packages.kubecove}/bin/kubecove";
            meta = packages.kubecove.meta;
          };
        in
        {
          default = app;
          kubecove = app;
        }
      );

      checks = eachSystem (
        system:
        let
          packages = packagesFor system;
        in
        {
          inherit (packages) frontend kubecove;
        }
      );
    };
}
