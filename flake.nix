{
  inputs = {
    utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages."${system}";
      naersk-lib = naersk.lib."${system}";
    in rec {
      # `nix build`
      packages.uefi-run = naersk-lib.buildPackage {
        pname = "uefi-run";
        root = ./.;
      };
      defaultPackage = packages.uefi-run;

      # `nix run`
      apps.uefi-run = utils.lib.mkApp {
        drv = packages.uefi-run;
      };
      defaultApp = apps.uefi-run;

      # `nix develop`
      devShell = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [ rustc cargo ];
      };
    });
}
